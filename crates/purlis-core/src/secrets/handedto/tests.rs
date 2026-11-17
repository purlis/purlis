// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

// Every value here is an obvious fake; none is or was a credential.

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
    let mut terminal = AlacrittyEngine::new(
        Size {
            columns: 80,
            rows: 10,
        },
        100,
    );
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
    remember(
        &root,
        41,
        &Entry::new("team", "github-token"),
        "FAKE-HANDED-VALUE-0007",
    )
    .unwrap();

    assert_eq!(
        masked_for(&root, 41, "x FAKE-HANDED-VALUE-0007").unwrap(),
        "x ‹secret: github-token›"
    );
    assert_eq!(
        masked_for(&root, 42, "x FAKE-HANDED-VALUE-0007").unwrap(),
        "x FAKE-HANDED-VALUE-0007"
    );
    assert_eq!(
        masked_for(&another, 41, "x FAKE-HANDED-VALUE-0007").unwrap(),
        "x FAKE-HANDED-VALUE-0007"
    );
}

#[test]
fn a_chat_forgotten_as_it_closes_masks_nothing_and_leaves_no_names_on_disk() {
    let (_dir, root) = a_project();
    remember(
        &root,
        43,
        &Entry::new("team", "k"),
        "FAKE-HANDED-VALUE-0008",
    )
    .unwrap();
    assert!(file(&root, 43).exists(), "named as it was handed");

    forget(&root, 43);

    assert_eq!(
        masked_for(&root, 43, "FAKE-HANDED-VALUE-0008").unwrap(),
        "FAKE-HANDED-VALUE-0008"
    );
    assert!(!file(&root, 43).exists());
}

#[test]
fn what_a_chat_was_handed_is_named_on_disk_by_vault_and_key_and_never_its_value() {
    let (_dir, root) = a_project();
    remember(
        &root,
        44,
        &Entry::new("team", "db-password"),
        "FAKE-HANDED-VALUE-0009",
    )
    .unwrap();
    remember(
        &root,
        44,
        &Entry::new("team", "db-password"),
        "FAKE-HANDED-VALUE-0009",
    )
    .unwrap();

    let written = std::fs::read_to_string(file(&root, 44)).expect("named");

    assert_eq!(written.lines().count(), 1, "{written}");
    assert!(
        written
            .starts_with("{\"vault\":\"team\",\"key\":\"db-password\",\"shown\":[{\"bytes\":22,"),
        "{written}"
    );
    assert!(!written.contains("FAKE-HANDED-VALUE-0009"), "{written}");
}

/// What a crashed window's process named on disk for chat `chat`, handed `value` as `entry`,
/// which this process never handed anything.
fn named_by_a_crashed_window(root: &Path, chat: u32, entry: Entry, value: &str) {
    remember(root, chat, &entry, value).expect("named");
    as_a_crash_leaves_it(root, chat);
}

#[test]
fn after_a_crash_a_value_named_on_disk_is_found_and_masked_with_no_vault_read() {
    let (_dir, root) = a_project();
    // No vault is registered: a launch finds the value by its line alone (#6907).
    named_by_a_crashed_window(
        &root,
        45,
        Entry::new("team", "api-key"),
        "FAKE-HANDED-VALUE-0010",
    );

    assert_eq!(
        masked_for(
            &root,
            45,
            "x FAKE-HANDED-VALUE-0010 and FAKE-HANDED-VALUE-0010"
        )
        .as_deref(),
        Some("x ‹secret: api-key› and ‹secret: api-key›")
    );
    assert_eq!(
        masked_for(&root, 45, "x FAKE-HANDED-VALUE-0011").as_deref(),
        Some("x FAKE-HANDED-VALUE-0011"),
        "a value of the same length that is not the one handed"
    );
}

#[test]
fn after_a_crash_a_value_of_several_lines_is_found_whole_and_line_by_line() {
    let (_dir, root) = a_project();
    named_by_a_crashed_window(
        &root,
        46,
        Entry::new("team", "cert"),
        "FAKE-CERT-LINE-0032\nFAKE-CERT-LINE-0033\n",
    );

    assert_eq!(
        masked_for(&root, 46, "FAKE-CERT-LINE-0032\r\nFAKE-CERT-LINE-0033\r\n").as_deref(),
        Some("‹secret: cert›\r\n")
    );
    assert_eq!(
        masked_for(&root, 46, "  FAKE-CERT-LINE-0033 alone").as_deref(),
        Some("  ‹secret: cert› alone")
    );
}

#[test]
fn an_entry_this_process_knows_is_masked_by_what_it_knows() {
    let (_dir, root) = a_project();
    remember(
        &root,
        47,
        &Entry::new("team", "k"),
        "FAKE-HANDED-VALUE-0011",
    )
    .unwrap();

    assert_eq!(
        masked_for(&root, 47, "FAKE-HANDED-VALUE-0011").as_deref(),
        Some("‹secret: k›")
    );
}

#[test]
fn names_left_30_days_are_swept_at_the_project_s_open_and_younger_ones_kept() {
    let (_dir, root) = a_project();
    named_by_a_crashed_window(&root, 48, Entry::new("team", "k"), "FAKE-HANDED-VALUE-0027");
    let now = std::time::SystemTime::now();

    on_open(
        &root,
        now + crate::retention::KEEP_FOR - std::time::Duration::from_secs(60),
        Some(&[]),
    );
    assert!(file(&root, 48).exists());

    on_open(
        &root,
        now + crate::retention::KEEP_FOR + std::time::Duration::from_secs(60),
        Some(&[]),
    );
    assert!(!file(&root, 48).exists());
}

#[test]
fn names_of_a_chat_the_plane_brings_back_are_kept_however_old() {
    let (_dir, root) = a_project();
    named_by_a_crashed_window(&root, 52, Entry::new("team", "k"), "FAKE-HANDED-VALUE-0028");
    let later = std::time::SystemTime::now()
        + crate::retention::KEEP_FOR
        + std::time::Duration::from_secs(60);

    on_open(&root, later, Some(&[52]));
    assert!(file(&root, 52).exists(), "a chat still open");

    on_open(&root, later, None);
    assert!(file(&root, 52).exists(), "no record says which are open");
}

#[test]
fn names_on_disk_that_do_not_read_leave_the_screen_unmaskable() {
    let (_dir, root) = a_project();
    named_lines_by_a_crashed_window(
        &root,
        53,
        "{\"vault\":\"team\",\"key\":\"k\"}\nnot a line\n",
    );
    assert_eq!(
        masked_for(&root, 53, "x"),
        None,
        "a line that does not parse"
    );

    std::fs::create_dir_all(file(&root, 54)).expect("a folder where the file goes");
    assert_eq!(
        masked_for(&root, 54, "x"),
        None,
        "a file that does not read"
    );
}

#[test]
fn a_value_whose_entry_cannot_be_named_on_disk_is_said_and_named_at_the_next_hand_out() {
    let (_dir, root) = a_project();
    std::fs::create_dir_all(file(&root, 55)).expect("a folder where the file goes");

    assert!(
        remember(
            &root,
            55,
            &Entry::new("team", "k"),
            "FAKE-HANDED-VALUE-0024"
        )
        .is_err()
    );

    std::fs::remove_dir(file(&root, 55)).expect("gone");
    remember(
        &root,
        55,
        &Entry::new("team", "k"),
        "FAKE-HANDED-VALUE-0024",
    )
    .unwrap();
    let written = std::fs::read_to_string(file(&root, 55)).expect("named");
    assert!(
        written.starts_with("{\"vault\":\"team\",\"key\":\"k\",\"shown\":[{\"bytes\":22,"),
        "{written}"
    );
}

#[test]
fn after_a_crash_an_entry_named_with_no_seal_of_purlis_s_leaves_the_screen_unmaskable() {
    let (_dir, root) = a_project();
    named_lines_by_a_crashed_window(&root, 57, "{\"vault\":\"team\",\"key\":\"k\"}\n");
    assert_eq!(masked_for(&root, 57, "x"), None, "no seal");

    named_lines_by_a_crashed_window(
        &root,
        57,
        &format!(
            "{{\"vault\":\"team\",\"key\":\"k\",\"shown\":[],\"seal\":\"{}\"}}\n",
            "0".repeat(64)
        ),
    );
    assert_eq!(
        masked_for(&root, 57, "x"),
        None,
        "a seal purlis did not make"
    );
}

#[test]
fn after_a_crash_a_line_changed_since_purlis_sealed_it_leaves_the_screen_unmaskable() {
    let (_dir, root) = a_project();
    named_by_a_crashed_window(
        &root,
        56,
        Entry::new("team", "api-key"),
        "FAKE-HANDED-VALUE-0029",
    );
    let written = std::fs::read_to_string(file(&root, 56)).expect("named");
    let mut line: serde_json::Value = serde_json::from_str(written.trim()).expect("a line");
    // A way it is shown taken out: the value would then go unmasked.
    line["shown"] = serde_json::json!([]);
    named_lines_by_a_crashed_window(&root, 56, &format!("{line}\n"));

    assert_eq!(masked_for(&root, 56, "x FAKE-HANDED-VALUE-0029"), None);
}

/// What a crashed window's process named on disk for chat `chat`: `lines`, as it wrote them.
fn named_lines_by_a_crashed_window(root: &Path, chat: u32, lines: &str) {
    std::fs::create_dir_all(dir(root)).expect("made");
    std::fs::write(file(root, chat), lines).expect("written");
}

#[test]
fn after_a_crash_a_value_handed_escaped_in_a_dotenv_file_is_masked_as_it_was_escaped() {
    let (_dir, root) = a_project();
    let cert = "FAKE-CERT-0016\nFAKE-CERT-0017";
    named_by_a_crashed_window(
        &root,
        49,
        Entry::new("team", "cert").in_form(Form::DotenvEscaped),
        &super::super::dotenv::escaped(cert),
    );

    assert_eq!(
        masked_for(&root, 49, "CERT=FAKE-CERT-0016\\nFAKE-CERT-0017\r\n").as_deref(),
        Some("CERT=‹secret: cert›\r\n")
    );
}

#[test]
fn after_a_crash_a_value_made_once_is_masked_as_it_was_handed() {
    let (_dir, root) = a_project();
    // A one-time code no launch could make again (#6907).
    named_by_a_crashed_window(
        &root,
        51,
        Entry::new("team", "totp").in_form(Form::Unrepeatable),
        "123456",
    );

    assert_eq!(
        masked_for(&root, 51, "code 123456").as_deref(),
        Some("code ‹secret: totp›")
    );
}

#[test]
fn what_is_named_on_disk_holds_no_value_and_no_two_of_its_prints_share_a_base() {
    let (_dir, root) = a_project();
    let value = "FAKE-HANDED-VALUE-0034\n";
    let named = Named::of(&root, Entry::new("team", "k"), value).expect("sealed");
    let line = serde_json::to_string(&named).expect("a line");
    assert!(!line.contains("FAKE-HANDED"), "{line}");

    let ctx = super::super::Ctx::new(&root, super::super::Env::from_process());
    let sought = named.sought(&ctx).expect("sealed by purlis");
    // As handed, and trimmed: one is the other with a line feed after it, and under one base
    // their two prints would say what the base is.
    assert_eq!(sought.len(), 2);
    assert_ne!(sought[0].base.base, sought[1].base.base);
}

#[test]
fn a_print_rolled_along_a_text_is_the_print_of_each_window_taken_alone() {
    let printed = Print::of_the_process().expect("a key for the process");
    let text = b"\x00\xffFAKE-ROLLING-0025 and on\x00";

    for length in [1, 4, 9, text.len()] {
        let rolled: Vec<u64> = printed.rolled(text, length).collect();
        let alone: Vec<u64> = text.windows(length).map(|run| printed.of(run)).collect();
        assert_eq!(rolled, alone, "windows of {length}");
    }
    assert_eq!(printed.rolled(text, text.len() + 1).count(), 0);
}

#[test]
fn a_screen_that_would_take_too_long_to_look_at_is_not_maskable() {
    let mut many = Vec::new();
    for length in 0..65 {
        many.extend(known("k", &"F".repeat(4 + length)));
    }
    let screen = "x".repeat(MATCH_STEPS_AT_MOST / 64);

    assert_eq!(masked(&screen, &many), None);
    assert!(masked("a screen of any length less", &many).is_some());
}
