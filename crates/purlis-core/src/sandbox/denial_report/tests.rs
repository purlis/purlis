// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{Reader, Receiver, Report, Tag, Totals, Unavailable, is_admin_in, tag_denials};

const CHAT: u32 = 2027;

fn tag() -> Tag {
    Tag::from_bytes([1, 2, 3, 4, 5, 6, 7, 8])
}

/// A record as `log stream --style compact` prints it: the header, then the tag.
fn record(writer: &str, sender: &str, pid: u32, message: &str, tag: &str) -> String {
    format!(
        "2026-11-21 16:44:02.146 E  {writer} ({sender}) {message}\n{tag}\n",
        message = message.replace("{pid}", &pid.to_string())
    )
}

fn deny(pid: u32, operation: &str, target: &str) -> String {
    record(
        "kernel[0:2324]",
        "Sandbox",
        pid,
        &format!("Sandbox: perl({{pid}}) deny(1) {operation} {target}"),
        tag().as_str(),
    )
}

fn folded(pid: u32, n: u32, target: &str) -> String {
    let reports = if n == 1 { "report" } else { "reports" };
    record(
        "kernel[0:2324]",
        "Sandbox",
        pid,
        &format!(
            "{n} duplicate {reports} for Sandbox: perl({{pid}}) deny(1) file-write-create {target}"
        ),
        tag().as_str(),
    )
}

fn read_all(text: &str) -> Vec<Report> {
    let mut reader = Reader::new(tag());
    text.lines().filter_map(|line| reader.feed(line)).collect()
}

#[test]
fn a_report_is_read_as_the_kernel_writes_it() {
    let reports = read_all(&deny(CHAT, "file-write-create", "/chat/dir/a file"));
    assert_eq!(
        reports,
        [Report {
            process: "perl".into(),
            pid: CHAT,
            operation: "file-write-create".into(),
            target: "/chat/dir/a file".into(),
            count: 1,
        }]
    );
}

#[test]
fn a_folded_report_stands_for_the_refusals_it_counts() {
    assert_eq!(read_all(&folded(CHAT, 18, "/chat/dir/rep"))[0].count, 18);
    assert_eq!(read_all(&folded(CHAT, 1, "/chat/dir/rep"))[0].count, 1);
}

#[test]
fn a_line_that_only_ends_in_the_tag_is_not_a_report() {
    // Anyone may write to the log: not the kernel, not `Sandbox`, or no tag on the next line.
    let spoofs = [
        record(
            "logger[88:9]",
            "Sandbox",
            CHAT,
            "Sandbox: perl({pid}) deny(1) file-write-create /x",
            tag().as_str(),
        ),
        record(
            "kernel[0:1]",
            "com.example.sub",
            CHAT,
            "Sandbox: perl({pid}) deny(1) file-write-create /x",
            tag().as_str(),
        ),
        record(
            "kernel[0:1]",
            "Sandbox",
            CHAT,
            "Sandbox: perl({pid}) deny(1) file-write-create /x",
            "purlis-denial-ffffffffffffffff",
        ),
        // The tag on the header line itself, no second line.
        format!(
            "2026-11-21 16:44:02.146 E  kernel[0:1] (Sandbox) Sandbox: perl(1) deny(1) file-write-create /x {}\n",
            tag().as_str()
        ),
        record(
            "kernel[0:1]",
            "Sandbox",
            CHAT,
            "not a refusal",
            tag().as_str(),
        ),
    ];
    for spoof in spoofs {
        assert_eq!(read_all(&spoof), [], "{spoof}");
    }
}

#[test]
fn every_deny_rule_is_tagged_and_nothing_else() {
    let profile = "(version 1)\n(deny default)\n(allow file-write* (subpath \"/a\"))\n\
                   (deny file-read-data file-write* (regex #\"^/x(y\\\"z)$\")) ; (deny hidden)\n";
    let tagged = tag_denials(profile, &tag());
    let message = "(with message \"purlis-denial-0102030405060708\")";
    assert_eq!(
        tagged,
        format!(
            "(version 1)\n(deny default {message})\n(allow file-write* (subpath \"/a\"))\n\
             (deny file-read-data file-write* {message} (regex #\"^/x(y\\\"z)$\")) ; (deny hidden)\n"
        )
    );
}

#[test]
fn an_unbalanced_profile_is_left_for_the_system_to_refuse() {
    assert_eq!(tag_denials("(deny default", &tag()), "(deny default");
}

#[test]
fn only_an_administrator_can_read_the_log() {
    assert!(is_admin_in("staff everyone localaccounts admin _appstore"));
    assert!(!is_admin_in("staff everyone localaccounts _lpoperator"));
}

#[test]
fn a_standard_user_is_told_there_is_no_receiver_in_purlis_words() {
    let said = Unavailable::NotAdmin.to_string();
    assert!(said.contains("not an administrator"), "{said}");
    assert!(said.contains("not reported on this account"), "{said}");
    assert!(said.contains("administrator"), "{said}");
}

/// A `log` that prints `output` and then stays until it is killed, as `log stream` does.
#[cfg(unix)]
fn fake_log(dir: &std::path::Path, output: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let fixture = dir.join("fixture");
    std::fs::write(
        &fixture,
        format!("Filtering the log data using ...\n{output}"),
    )
    .unwrap();
    let program = dir.join("log");
    std::fs::write(
        &program,
        format!("#!/bin/sh\ncat '{}'\nexec sleep 60\n", fixture.display()),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    program
}

#[cfg(unix)]
type Told = Arc<Mutex<Vec<(String, String)>>>;

/// Runs the receiver over `output`, with the pids in `chat` the chat's: what it told and totals.
#[cfg(unix)]
fn received(output: &str, chat: &'static [u32]) -> (Vec<(String, String)>, Totals) {
    let dir = tempfile::tempdir().unwrap();
    let program = fake_log(dir.path(), output);
    let told: Told = Arc::default();
    let sink = Arc::clone(&told);
    let receiver = Receiver::start_with(
        &program,
        &tag(),
        move |pid| chat.contains(&pid),
        Box::new(move |refusal| {
            sink.lock()
                .unwrap()
                .push((refusal.operation.to_owned(), refusal.target.clone()));
        }),
    )
    .expect("the receiver starts");
    let totals = receiver.finish(Duration::from_millis(500));
    let told = told.lock().unwrap().clone();
    (told, totals)
}

#[cfg(unix)]
#[test]
fn a_seatbelt_denial_for_the_chats_process_becomes_a_refusal() {
    let (told, totals) = received(&deny(CHAT, "file-write-create", "/chat/dir/a"), &[CHAT]);
    assert_eq!(
        told,
        [("file-write-create".to_owned(), "/chat/dir/a".to_owned())]
    );
    assert_eq!(totals.told, 1);
}

#[cfg(unix)]
#[test]
fn another_processes_denial_is_never_attributed_to_the_chat() {
    let output = format!(
        "{}{}{}",
        deny(999, "file-write-create", "/other/a"),
        folded(999, 5, "/other/b"),
        deny(CHAT, "file-write-create", "/chat/dir/a"),
    );
    let (told, totals) = received(&output, &[CHAT]);
    assert_eq!(
        told,
        [("file-write-create".to_owned(), "/chat/dir/a".to_owned())]
    );
    assert_eq!(totals.unattributed, 6);
}

#[cfg(unix)]
#[test]
fn a_same_target_retry_on_macos_is_still_reported() {
    // Three lines that stood alone and a fold of five: eight refusals of one file.
    let output = format!(
        "{}{}{}{}",
        deny(CHAT, "file-write-create", "/chat/dir/a"),
        deny(CHAT, "file-write-create", "/chat/dir/a"),
        deny(CHAT, "file-write-create", "/chat/dir/a"),
        folded(CHAT, 5, "/chat/dir/a"),
    );
    let (told, _) = received(&output, &[CHAT]);
    assert!(told.len() >= 8, "{} told", told.len());
}

#[cfg(unix)]
#[test]
fn a_fold_for_a_process_that_has_ended_is_still_told() {
    // The chat's pid is the chat's only while it lives; its late fold is told all the same.
    static ALIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
    let dir = tempfile::tempdir().unwrap();
    let output = format!(
        "{}{}",
        deny(CHAT, "file-write-create", "/chat/dir/a"),
        folded(CHAT, 4, "/chat/dir/a")
    );
    let program = fake_log(dir.path(), &output);
    let told: Told = Arc::default();
    let sink = Arc::clone(&told);
    let receiver = Receiver::start_with(
        &program,
        &tag(),
        |pid| pid == CHAT && ALIVE.swap(false, std::sync::atomic::Ordering::SeqCst),
        Box::new(move |refusal| {
            sink.lock()
                .unwrap()
                .push((refusal.operation.to_owned(), refusal.target.clone()));
        }),
    )
    .unwrap();
    let totals = receiver.finish(Duration::from_millis(500));
    assert_eq!(totals.told, 5);
}

#[cfg(unix)]
#[test]
fn an_eleventh_distinct_refusal_in_a_minute_is_still_reported_on_macos() {
    let output: String = (0..11)
        .map(|n| deny(CHAT, "file-write-create", &format!("/chat/dir/{n}")))
        .collect();
    let (told, _) = received(&output, &[CHAT]);
    assert_eq!(told.len(), 11);
}

#[cfg(unix)]
#[test]
fn a_burst_of_denials_on_macos_is_all_reported() {
    // 100 refusals of one file as the kernel reports them: 84 lines and folds of 16 in all.
    let mut output = String::new();
    for _ in 0..84 {
        output.push_str(&deny(CHAT, "file-write-create", "/chat/dir/rep"));
    }
    for _ in 0..4 {
        output.push_str(&folded(CHAT, 4, "/chat/dir/rep"));
    }
    let (told, totals) = received(&output, &[CHAT]);
    assert!(told.len() >= 100, "{} told", told.len());
    assert_eq!(totals.told, 100);
}

#[cfg(unix)]
#[test]
fn an_operation_without_a_word_is_counted_not_told() {
    let output = deny(CHAT, "iokit-open", "/dev/x");
    let (told, totals) = received(&output, &[CHAT]);
    assert_eq!(told, []);
    assert_eq!(totals.unrecognised, 1);
}

#[cfg(unix)]
#[test]
fn a_stream_that_ends_at_once_is_no_receiver() {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("log");
    std::fs::write(
        &program,
        "#!/bin/sh\necho 'log: Must be admin' >&2\nexit 1\n",
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let started = Receiver::start_with(&program, &tag(), |_| true, Box::new(|_| {}));
    match started {
        Err(Unavailable::NotStarted(why)) => assert!(why.contains("Must be admin"), "{why}"),
        other => panic!("expected no receiver: {:?}", other.err()),
    }
}
