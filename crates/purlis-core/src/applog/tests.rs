// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn an_event_holding_a_kept_out_value_with_no_known_shape_is_redacted() {
    // No prefix and no shape any rule knows: only the value itself can catch it.
    let token = "plain-words-held-for-the-forge";
    let event = format!("purlis: the forge answered {token} today\n");
    assert_eq!(redacted(&event), event, "nothing is kept out yet");
    keep_out(&secrecy::SecretString::from(token));
    assert_eq!(
        redacted(&event),
        "[redacted: an event that looks like it holds a forge token purlis holds]\n"
    );
    assert_eq!(
        without_values_kept_out(&event),
        "purlis: the forge answered [a token purlis holds] today\n"
    );
}

#[test]
fn an_event_holding_a_vault_value_of_any_shape_is_redacted() {
    // #1872: plain words, built from parts, that only purlis's memory of the value finds.
    let value = format!("{}-{}-{}", "plain", "words from the vault", "e51c");
    let event = format!("purlis: the server said {value} back\n");
    assert_eq!(redacted(&event), event, "nothing is known of it yet");
    crate::secrets::seen::remember(&value);
    assert_eq!(
        redacted(&event),
        "[redacted: an event that looks like it holds a vault value purlis read]\n"
    );
}

/// Set by the thread [`redaction_made_ready_beside_the_launch`] starts once it has built
/// the rules, so a test can show it logged before they were ready.
pub(super) static RULES_BUILT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Marks the child [`the_first_event_of_a_launch_is_redacted_while_the_rules_are_still_building`]
/// starts: a process whose rules nobody has built yet.
const FRESH_CHILD: &str = "APPLOG_TEST_FRESH_PROCESS";

/// A standard error a test can read back.
#[derive(Clone, Default)]
struct Screen(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl Screen {
    fn said(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl Write for Screen {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The subscriber, with standard error kept off the test run's own.
fn quiet(dir: &Path) -> impl tracing::Subscriber + Send + Sync {
    subscriber_to(Some(daily_file(dir).unwrap()), Screen::default)
}

/// Everything written to the log files in `dir`, oldest file first.
fn logged(dir: &Path) -> String {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("the log directory exists")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    files.sort();
    files
        .iter()
        .map(|file| std::fs::read_to_string(file).expect("a log file reads"))
        .collect()
}

#[test]
fn a_warning_from_charter_lands_in_a_file_in_the_log_directory() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");

    tracing::subscriber::with_default(quiet(&logs), || {
        tracing::warn!("purlis: no tray icon (the desktop has none)");
    });

    let said = logged(&logs);
    assert!(
        said.contains("purlis: no tray icon (the desktop has none)"),
        "{said:?}"
    );
    assert!(said.contains("WARN"), "the file names the level: {said:?}");
}

#[test]
fn an_event_that_carries_a_secret_never_reaches_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    let token = concat!("ghp_", "0123456789abcdefghijABCDEFGHIJ012345");

    tracing::subscriber::with_default(quiet(&logs), || {
        tracing::warn!("charter: the forge answered 401 for {token}");
        tracing::warn!("charter: password: hunter2is was refused");
        tracing::warn!("charter: the next line is still written");
    });

    let said = logged(&logs);
    assert!(
        !said.contains(token),
        "the token was written down: {said:?}"
    );
    assert!(
        !said.contains("hunter2is"),
        "the password was written down: {said:?}"
    );
    assert!(
        said.contains(
            "[redacted: an event that looks like it holds a token by its forge's prefix]"
        ),
        "the redaction says what kind it removed, so the gap is not a mystery: {said:?}"
    );
    assert!(
        said.contains("charter: the next line is still written"),
        "one redaction does not silence the log: {said:?}"
    );
}

#[test]
fn standard_error_still_gets_the_message_as_it_was_printed_and_redacted_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let screen = Screen::default();
    let token = concat!("ghp_", "0123456789abcdefghijABCDEFGHIJ012345");

    let seen = screen.clone();
    tracing::subscriber::with_default(
        subscriber_to(Some(daily_file(dir.path()).unwrap()), move || seen.clone()),
        || {
            tracing::warn!("purlis: no tray icon; its menu is still on the click");
            tracing::warn!("purlis: the forge answered 401 for {token}");
        },
    );

    assert_eq!(
        screen.said(),
        "purlis: no tray icon; its menu is still on the click\n\
         [redacted: an event that looks like it holds a token by its forge's prefix]\n",
        "a person running the app from a terminal reads what eprintln! printed, and no \
         more; the time and level are the file's"
    );
}

#[test]
fn charter_says_what_it_noticed_and_a_library_only_what_went_wrong() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");

    tracing::subscriber::with_default(quiet(&logs), || {
        tracing::info!("purlis: plane /p, 2 of 2 chats back");
        tracing::debug!("charter: a detail nobody asked for");
        tracing::info!(target: "zbus::connection", "a library's routine chatter");
        tracing::warn!(target: "zbus::connection", "a library's own trouble");
    });

    let said = logged(&logs);
    assert!(said.contains("2 of 2 chats back"), "{said:?}");
    assert!(said.contains("a library's own trouble"), "{said:?}");
    assert!(!said.contains("a detail nobody asked for"), "{said:?}");
    assert!(!said.contains("routine chatter"), "{said:?}");
}

#[test]
fn a_week_of_logs_is_kept_and_the_panic_log_beside_it_is_never_touched() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path();
    std::fs::write(logs.join("panics.log"), "charter-panic\n").unwrap();
    // Nine days of logs, oldest first: the appender orders them by when each was made.
    for day in 1..=9 {
        std::fs::write(logs.join(format!("charter.2026-09-0{day}.log")), "a day\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    tracing::subscriber::with_default(quiet(logs), || {
        tracing::warn!("charter: today");
    });

    let mut kept: Vec<String> = std::fs::read_dir(logs)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("charter."))
        .collect();
    kept.sort();
    assert_eq!(kept.len(), 7, "today and the six days before it: {kept:?}");
    assert!(
        !kept.contains(&"charter.2026-09-03.log".to_owned()),
        "the oldest days went first: {kept:?}"
    );
    assert!(
        logs.join("panics.log").exists(),
        "the panic log is not the appender's"
    );
}

#[test]
fn the_log_is_kept_beside_the_panic_log_unless_the_environment_names_a_directory() {
    let app = crate::report::panic_log().and_then(|file| file.parent().map(Path::to_path_buf));
    // Under `$CHARTER_PANIC_LOG` the panic log is wherever that says, so only compare
    // with the app's own directory when it is the one the panic log is in.
    if crate::envvar::var_os("PURLIS_PANIC_LOG").is_none() {
        assert_eq!(
            dir_from(None),
            app,
            "the same directory `charter report` reads"
        );
    }
    assert_eq!(
        dir_from(Some("".into())),
        app_log_dir(),
        "an empty value names nothing"
    );
    assert_eq!(
        dir_from(Some("/elsewhere/logs".into())),
        Some(std::path::PathBuf::from("/elsewhere/logs"))
    );
    let app = app_log_dir().expect("a home to find it under");
    let app = app.to_string_lossy();
    assert!(
        ["dev.purlis.app", "dev.charter.app"].iter().any(|id| {
            app.ends_with(&format!("Library/Logs/{id}")) || app.ends_with(&format!("{id}/logs"))
        }),
        "Tauri's `app_log_dir` for the app's identifier, or the old one's: {app}"
    );
}

/// What `events` leave in the file, each raised as a warning from charter.
fn written(events: &[&str]) -> String {
    let dir = tempfile::tempdir().unwrap();
    tracing::subscriber::with_default(quiet(dir.path()), || {
        for event in events {
            tracing::warn!("charter: {event}");
        }
    });
    logged(dir.path())
}

/// The rules are built beside the launch (#1804), so the launch's first event can be raised
/// before they are ready. It must wait for them, never be written as it was said. Rules are
/// built once a process, and other tests here have built them already, so this runs in a
/// child of the test binary whose first event IS the launch's first.
#[test]
fn the_first_event_of_a_launch_is_redacted_while_the_rules_are_still_building() {
    crate::testrun::rerun(
        &["applog::tests::in_a_fresh_process_the_first_event_waits_for_the_rules"],
        &[(FRESH_CHILD, std::ffi::OsStr::new("1"))],
    );
}

/// The child's half of the test above; outside that child it has nothing to show.
#[test]
fn in_a_fresh_process_the_first_event_waits_for_the_rules() {
    if std::env::var_os(FRESH_CHILD).is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    let token = concat!("ghp_", "0123456789abcdefghijABCDEFGHIJ012345");
    let screen = Screen::default();
    let seen = screen.clone();

    let subscriber = subscriber_to(Some(daily_file(&logs).unwrap()), move || seen.clone());
    redaction_made_ready_beside_the_launch();
    let building = !RULES_BUILT.load(std::sync::atomic::Ordering::SeqCst);
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!("purlis: the forge answered 401 for {token}");
    });

    assert!(
        building,
        "the rules were ready before the first event, so this proves nothing"
    );
    for (whence, said) in [
        ("the file", logged(&logs)),
        ("standard error", screen.said()),
    ] {
        assert!(!said.contains(token), "{whence} holds the token: {said:?}");
        assert!(
            said.contains("[redacted: an event that looks like it holds"),
            "{whence}: {said:?}"
        );
    }
}

#[test]
fn a_vendor_key_gitleaks_knows_never_reaches_the_file() {
    let key = concat!("lin_api_", "0123456789abcdefghijABCDEFGHIJ0123456789");
    let said = written(&[&format!("the tracker refused {key}")]);
    assert!(!said.contains(key), "{said:?}");
    assert!(
        said.contains("[redacted: an event that looks like it holds Linear key]"),
        "{said:?}"
    );
}

#[test]
fn personal_data_never_reaches_the_file() {
    // A real-looking address, put together as the test runs: an address on a reserved
    // domain names nobody, and the log rightly lets it through.
    let address = ["jane.doe", "@", "acme-corp.io"].concat();
    let said = written(&[&format!("the commit was authored by {address}")]);
    assert!(!said.contains(&address), "{said:?}");
    assert!(
        said.contains("[redacted: an event that looks like it holds"),
        "{said:?}"
    );
}

#[test]
fn a_field_named_for_a_token_or_a_secret_never_reaches_the_file() {
    for field in [
        "access_token=Zq8xV3mN0pRt",
        "refresh_token: Zq8xV3mN0pRt",
        "auth_token=Zq8xV3mN0pRt",
        "client_secret=Zq8xV3mN0pRt",
        "webhook_secret = Zq8xV3mN0pRt",
        r#"{"access_token":"Zq8xV3mN0pRt"}"#,
    ] {
        let said = written(&[&format!("the forge answered {field}")]);
        assert!(!said.contains("Zq8xV3mN0pRt"), "{field}: {said:?}");
        assert!(
            said.contains("[redacted: an event that looks like it holds"),
            "{field}: {said:?}"
        );
    }
}

#[test]
fn a_word_that_only_mentions_a_token_is_still_written() {
    let said = written(&["the access_token field was missing from the answer"]);
    assert!(
        said.contains("the access_token field was missing"),
        "{said:?}"
    );
}

#[test]
fn under_the_cli_only_charters_warnings_reach_the_terminal() {
    let screen = Screen::default();
    let seen = screen.clone();
    tracing::subscriber::with_default(terminal_only(move || seen.clone()), || {
        tracing::info!("charter: narration the terminal never asked for");
        tracing::warn!(target: "zbus::connection", "a library's own trouble");
        tracing::warn!("charter: the core noticed something");
    });
    assert_eq!(screen.said(), "charter: the core noticed something\n");
}

#[test]
fn an_event_that_spells_a_secret_through_its_escapes_is_redacted() {
    let tail = &crate::secretshape::escaped::token()[1..];
    for event in [
        format!("charter: the forge said {{\"t\": \"\\u0067{tail}\"}}\n"),
        format!(
            "charter: {{\"owner\": \"\\u0061{}\"}}\n",
            ["da", "@", "lovelace.dev"].concat()
        ),
    ] {
        let said = redacted(&event);
        assert!(said.starts_with("[redacted: "), "{said}");
    }
}
