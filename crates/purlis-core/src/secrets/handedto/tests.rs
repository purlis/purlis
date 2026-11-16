// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

// Every value here is an obvious fake; none is or was a credential.

/// A reader that reads no entry again.
fn nothing(_: &Entry) -> Option<zeroize::Zeroizing<String>> {
    None
}

fn known(name: &str, value: &str) -> Vec<Known> {
    Known::of(name, value).expect("a key for the process")
}

#[test]
fn a_value_on_a_screen_is_shown_as_its_entry_s_named_mask() {
    let screen = "$ deploy\r\ntoken is FAKE-VAULT-VALUE-0001, done\r\n";

    let masked = masked(screen, &known("github-token", "FAKE-VAULT-VALUE-0001")).unwrap();

    assert_eq!(
        masked,
        "$ deploy\r\ntoken is ‹secret: github-token›, done\r\n"
    );
}

#[test]
fn a_value_a_colour_change_runs_through_is_masked_and_the_colour_kept() {
    // The snapshot writes a soft-wrapped line as one run of cells, and a pen change between
    // two of them as a sequence: the value is still one word on the screen.
    let screen = "key: FAKE-VAULT-\x1b[1mVALUE-0002\x1b[0m\r\n";

    let masked = masked(screen, &known("db-password", "FAKE-VAULT-VALUE-0002")).unwrap();

    assert_eq!(masked, "key: ‹secret: db-password›\x1b[1m\x1b[0m\r\n");
}

#[test]
fn a_value_broken_by_a_hard_line_break_is_two_words_and_is_not_masked() {
    let screen = "FAKE-VAULT-\r\nVALUE-0003\r\n";

    let masked = masked(screen, &known("api-key", "FAKE-VAULT-VALUE-0003")).unwrap();

    assert_eq!(masked, screen);
}

#[test]
fn a_value_of_several_lines_printed_by_cat_is_masked_whole() {
    // `cat` of a credential file: the terminal shows each of its lines on a row of its own, each
    // row ended as a terminal ends one, with a carriage return.
    let value = "FAKE-LINE-ONE-0012\nFAKE-LINE-TWO-0012\n";
    let screen = "$ cat kubeconfig\r\nFAKE-LINE-ONE-0012\r\nFAKE-LINE-TWO-0012\r\n$ ";

    let masked = masked(screen, &known("kubeconfig", value)).unwrap();

    assert_eq!(masked, "$ cat kubeconfig\r\n‹secret: kubeconfig›\r\n$ ");
}

#[test]
fn a_value_of_several_lines_cat_printed_is_masked_in_the_terminal_s_own_snapshot() {
    use crate::engine::{AlacrittyEngine, Engine, Size};
    let value = "FAKE-LINE-ONE-0014\nFAKE-LINE-TWO-0014\n";
    let mut terminal = AlacrittyEngine::new(Size { columns: 80, rows: 10 }, 100);
    // What `cat` wrote, as the terminal's line discipline hands it on (ONLCR).
    terminal.advance(b"$ cat kubeconfig\r\n");
    terminal.advance(value.replace('\n', "\r\n").as_bytes());
    let snapshot = String::from_utf8(terminal.snapshot()).expect("text");

    let masked = masked(&snapshot, &known("kubeconfig", value)).unwrap();

    assert!(!masked.contains("FAKE-LINE"), "{masked:?}");
    assert!(masked.contains("‹secret: kubeconfig›"), "{masked:?}");
}

#[test]
fn each_line_of_a_value_of_several_lines_is_masked_where_it_is_shown_alone() {
    // A harness that indents what a command printed breaks the value up: each line is still
    // the value's, and is masked where it stands.
    let value = "FAKE-LINE-ONE-0013\r\n  FAKE-LINE-TWO-0013\r\n";
    let screen = "  │ FAKE-LINE-TWO-0013\r\n  │ FAKE-LINE-ONE-0013\r\n";

    let masked = masked(screen, &known("service-account", value)).unwrap();

    assert_eq!(
        masked,
        "  │ ‹secret: service-account›\r\n  │ ‹secret: service-account›\r\n"
    );
}

#[test]
fn every_occurrence_is_masked_and_text_around_is_untouched() {
    let screen = "a FAKE-VALUE-0004 b FAKE-VALUE-0004 c ‹é›";

    let masked = masked(screen, &known("k", "FAKE-VALUE-0004")).unwrap();

    assert_eq!(masked, "a ‹secret: k› b ‹secret: k› c ‹é›");
}

#[test]
fn two_values_that_overlap_are_masked_whole_each_named_in_the_order_they_start() {
    let mut both = known("first", "FAKE-AAAA-BBBB");
    both.extend(known("second", "BBBB-CCCC-FAKE"));
    let screen = "[FAKE-AAAA-BBBB-CCCC-FAKE]";

    let masked = masked(screen, &both).unwrap();

    assert_eq!(masked, "[‹secret: first›‹secret: second›]");
}

#[test]
fn a_value_read_from_a_file_with_a_trailing_newline_is_masked_as_printed() {
    let screen = "pw=FAKE-FILE-VALUE-0005\r\n";

    let masked = masked(screen, &known("pw", "FAKE-FILE-VALUE-0005\n")).unwrap();

    assert_eq!(masked, "pw=‹secret: pw›\r\n");
}

#[test]
fn a_value_shorter_than_four_bytes_is_not_masked() {
    assert!(known("pin", "abc").is_empty());
    assert_eq!(masked("abc abc", &known("pin", "abc")).unwrap(), "abc abc");
}

#[test]
fn a_value_inside_an_osc_title_sequence_is_masked_in_the_title_text_too() {
    // A window title is text the terminal shows, and a kept screen should not carry it.
    let screen = "\x1b]0;FAKE-TITLE-VALUE-06\x07ok";

    let masked = masked(screen, &known("t", "FAKE-TITLE-VALUE-06")).unwrap();

    assert!(!masked.contains("FAKE-TITLE-VALUE-06"), "{masked:?}");
}

/// A project folder of its own: what a chat was handed is named on disk under it.
fn a_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a folder");
    let root = std::fs::canonicalize(dir.path()).expect("canonical");
    (dir, root)
}

#[test]
fn what_a_chat_was_handed_masks_its_screen_and_no_other_chat_s() {
    let (_dir, root) = a_project();
    let (_other_dir, another) = a_project();
    remember(&root, 41, &Entry::new("team", "github-token"), "FAKE-HANDED-VALUE-0007");

    assert_eq!(
        masked_for(&root, 41, "x FAKE-HANDED-VALUE-0007", &nothing).unwrap(),
        "x ‹secret: github-token›"
    );
    assert_eq!(
        masked_for(&root, 42, "x FAKE-HANDED-VALUE-0007", &nothing).unwrap(),
        "x FAKE-HANDED-VALUE-0007"
    );
    assert_eq!(
        masked_for(&another, 41, "x FAKE-HANDED-VALUE-0007", &nothing).unwrap(),
        "x FAKE-HANDED-VALUE-0007"
    );
}

#[test]
fn a_chat_forgotten_as_it_closes_masks_nothing_and_leaves_no_names_on_disk() {
    let (_dir, root) = a_project();
    remember(&root, 43, &Entry::new("team", "k"), "FAKE-HANDED-VALUE-0008");
    assert!(file(&root, 43).exists(), "named as it was handed");

    forget(&root, 43);

    assert_eq!(
        masked_for(&root, 43, "FAKE-HANDED-VALUE-0008", &nothing).unwrap(),
        "FAKE-HANDED-VALUE-0008"
    );
    assert!(!file(&root, 43).exists());
}

#[test]
fn what_a_chat_was_handed_is_named_on_disk_by_vault_and_key_and_never_its_value() {
    let (_dir, root) = a_project();
    remember(&root, 44, &Entry::new("team", "db-password"), "FAKE-HANDED-VALUE-0009");
    remember(&root, 44, &Entry::new("team", "db-password"), "FAKE-HANDED-VALUE-0009");

    let written = std::fs::read_to_string(file(&root, 44)).expect("named");

    assert_eq!(written, "{\"vault\":\"team\",\"key\":\"db-password\"}\n");
}

/// What a crashed window's process named on disk for chat `chat`, which this process never
/// handed anything.
fn named_by_a_crashed_window(root: &Path, chat: u32, vault: &str, key: &str) {
    std::fs::create_dir_all(dir(root)).expect("made");
    std::fs::write(
        file(root, chat),
        format!("{{\"vault\":\"{vault}\",\"key\":\"{key}\"}}\n"),
    )
    .expect("written");
}

#[test]
fn after_a_crash_an_entry_named_on_disk_is_read_again_and_masked() {
    let (_dir, root) = a_project();
    named_by_a_crashed_window(&root, 45, "team", "api-key");
    let read = |entry: &Entry| {
        (entry == &Entry::new("team", "api-key"))
            .then(|| zeroize::Zeroizing::new("FAKE-HANDED-VALUE-0010".to_owned()))
    };

    assert_eq!(
        masked_for(&root, 45, "x FAKE-HANDED-VALUE-0010", &read).as_deref(),
        Some("x ‹secret: api-key›")
    );
}

#[test]
fn after_a_crash_an_entry_that_cannot_be_read_again_leaves_the_screen_unmaskable() {
    let (_dir, root) = a_project();
    named_by_a_crashed_window(&root, 46, "team", "minted-token");

    assert_eq!(masked_for(&root, 46, "x anything", &nothing), None);
}

#[test]
fn an_entry_this_process_knows_is_not_read_again() {
    let (_dir, root) = a_project();
    remember(&root, 47, &Entry::new("team", "k"), "FAKE-HANDED-VALUE-0011");

    assert_eq!(
        masked_for(&root, 47, "FAKE-HANDED-VALUE-0011", &|_| panic!(
            "read again"
        ))
        .as_deref(),
        Some("‹secret: k›")
    );
}

#[test]
fn names_left_30_days_are_swept_at_the_project_s_open_and_younger_ones_kept() {
    let (_dir, root) = a_project();
    named_by_a_crashed_window(&root, 48, "team", "k");
    let now = std::time::SystemTime::now();

    on_open(
        &root,
        now + crate::retention::KEEP_FOR - std::time::Duration::from_secs(60),
    );
    assert!(file(&root, 48).exists());

    on_open(
        &root,
        now + crate::retention::KEEP_FOR + std::time::Duration::from_secs(60),
    );
    assert!(!file(&root, 48).exists());
}

/// What a crashed window's process named on disk for chat `chat`: `lines`, as it wrote them.
fn named_lines_by_a_crashed_window(root: &Path, chat: u32, lines: &str) {
    std::fs::create_dir_all(dir(root)).expect("made");
    std::fs::write(file(root, chat), lines).expect("written");
}

#[test]
fn after_a_crash_a_value_handed_escaped_in_a_dotenv_file_is_masked_as_it_was_escaped() {
    let (_dir, root) = a_project();
    named_lines_by_a_crashed_window(
        &root,
        49,
        "{\"vault\":\"team\",\"key\":\"cert\"}\n\
         {\"vault\":\"team\",\"key\":\"cert\",\"form\":\"dotenv-escaped\"}\n",
    );
    let read = |_: &Entry| Some(zeroize::Zeroizing::new("FAKE-CERT-0016\nFAKE-CERT-0017".into()));

    assert_eq!(
        masked_for(&root, 49, "CERT=FAKE-CERT-0016\\nFAKE-CERT-0017\r\n", &read).as_deref(),
        Some("CERT=‹secret: cert›\r\n")
    );
}

#[test]
fn after_a_crash_a_value_pointed_at_a_tunnel_is_masked_as_it_was_pointed() {
    let (_dir, root) = a_project();
    let form = Form::Tunnelled {
        port: 54321,
        pgsslmode: None,
        reads_hostaddr: true,
        escaped: false,
    };
    named_lines_by_a_crashed_window(
        &root,
        50,
        &format!(
            "{}\n",
            serde_json::to_string(&Entry::new("team", "dsn").in_form(form)).unwrap()
        ),
    );
    let read = |_: &Entry| {
        Some(zeroize::Zeroizing::new(
            "postgres://fake-user:FAKE-PASS-0018@db.example.test:5432/orders".into(),
        ))
    };
    let shown = "connecting to postgres://fake-user:FAKE-PASS-0018@127.0.0.1:54321/orders\r\n";

    assert_eq!(
        masked_for(&root, 50, shown, &read).as_deref(),
        Some("connecting to ‹secret: dsn›\r\n")
    );
}

#[test]
fn after_a_crash_a_value_made_once_leaves_the_screen_unmaskable() {
    let (_dir, root) = a_project();
    named_lines_by_a_crashed_window(
        &root,
        51,
        "{\"vault\":\"team\",\"key\":\"totp\",\"form\":\"unrepeatable\"}\n",
    );
    let read = |_: &Entry| Some(zeroize::Zeroizing::new("FAKE-SEED-0019".into()));

    assert_eq!(masked_for(&root, 51, "code 123456", &read), None);
}
