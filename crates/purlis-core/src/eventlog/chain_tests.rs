// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;
use crate::eventlog::{Log, read};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";

fn append(log: &mut Log, kind: &str, n: u64) {
    log.append(
        Some("chat"),
        None,
        None,
        kind,
        serde_json::json!({ "n": n }),
    )
    .unwrap();
}

/// A chained log of `lines` events, closed.
fn chained_log(dir: &Path, lines: u64) {
    let mut log = Log::open(dir, DEVICE).unwrap();
    log.chain().unwrap();
    for n in 0..lines {
        append(&mut log, "tool.called", n);
    }
}

fn lines(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join(FILE))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn write_lines(dir: &Path, lines: &[String]) {
    std::fs::write(dir.join(FILE), lines.join("\n") + "\n").unwrap();
}

/// With the sandbox off the host chains the log: each line ends in the chain of the one before
/// it, a reader that knows nothing of the chain reads the same events, and a line edited in place
/// is the first that no longer matches.
#[test]
fn with_the_sandbox_off_the_event_log_is_hash_chained() {
    let dir = tempfile::tempdir().unwrap();
    chained_log(dir.path(), 4);
    let written = lines(dir.path());
    assert_eq!(written.len(), 4);
    let mut previous = String::new();
    for line in &written {
        let (over, chain) = split(line.as_bytes()).expect("every line carries its chain");
        assert_eq!(link(&previous, &over), chain, "{line}");
        previous = chain.to_owned();
    }
    assert_eq!(
        read(dir.path()).unwrap().len(),
        4,
        "the chain is no field of the event: a reader reads each event as before"
    );
    assert!(matches!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Sealed { seq: 4 }
    ));

    // An edit of the third line, its chain left alone: that line is the first that no longer
    // matches.
    let mut edited = written.clone();
    edited[2] = edited[2].replace("\"n\":2", "\"n\":99");
    write_lines(dir.path(), &edited);
    assert_eq!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Broken {
            seq: 3,
            why: Why::ChainDoesNotHold
        }
    );

    // A line taken out, and a line with its chain taken off, are named too.
    let mut dropped = written.clone();
    dropped.remove(1);
    write_lines(dir.path(), &dropped);
    assert_eq!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Broken {
            seq: 3,
            why: Why::OutOfOrder
        }
    );
    let mut bare = written.clone();
    let (over, _) = split(bare[3].as_bytes()).unwrap();
    bare[3] = String::from_utf8(over).unwrap();
    write_lines(dir.path(), &bare);
    assert_eq!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Broken {
            seq: 4,
            why: Why::NotChained
        }
    );
}

/// A log opened again carries on its chain, so the lines written after a restart are chained to
/// the ones before it; and a log that was never chained claims nothing.
#[test]
fn a_chain_carries_on_across_a_restart_and_a_log_never_chained_claims_nothing() {
    let dir = tempfile::tempdir().unwrap();
    chained_log(dir.path(), 2);
    {
        let mut log = Log::open(dir.path(), DEVICE).unwrap();
        assert!(log.is_chained(), "a chained log stays chained once opened");
        append(&mut log, "tool.called", 2);
    }
    assert!(matches!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Sealed { seq: 3 }
    ));

    // The first log's seals are another log's: a new folder for the device would read as deleted.
    crate::secrets::seal::forget_kept_for_this_test();
    let plain = tempfile::tempdir().unwrap();
    let mut log = Log::open(plain.path(), DEVICE).unwrap();
    append(&mut log, "tool.called", 0);
    assert_eq!(check(plain.path(), DEVICE).unwrap(), Checked::NotChained);
}

/// A bare chain can be recomputed by whoever rewrites the log. The head the app sealed cannot: a
/// log whose every chain holds, rewritten from its second line, still ends in a head the seal
/// does not hold, and the report names it.
#[test]
fn a_recomputed_hash_chain_is_still_reported() {
    let dir = tempfile::tempdir().unwrap();
    chained_log(dir.path(), 5);
    let written = lines(dir.path());
    // Rewrite the second line's event and recompute every chain from it to the end.
    let mut rewritten = Vec::new();
    let mut previous = String::new();
    for (at, line) in written.iter().enumerate() {
        let (over, _) = split(line.as_bytes()).unwrap();
        let mut over = String::from_utf8(over).unwrap();
        if at == 1 {
            over = over.replace("\"n\":1", "\"n\":777");
        }
        let (line, chain) = chained(&previous, over.into_bytes());
        previous = chain;
        rewritten.push(String::from_utf8(line).unwrap());
    }
    assert_ne!(rewritten, written);
    write_lines(dir.path(), &rewritten);
    let found = check(dir.path(), DEVICE).unwrap();
    assert_eq!(
        found,
        Checked::Broken {
            seq: 5,
            why: Why::HeadIsNotTheSealed
        },
        "every chain holds and the seal does not"
    );
    let said = found.said(dir.path()).expect("a break is said");
    assert!(said.contains("line 5"), "{said}");
    assert!(said.contains(&dir.path().display().to_string()), "{said}");

    // The tail cut off, with the chain intact to where it stops, is the same break.
    crate::secrets::seal::forget_kept_for_this_test();
    let cut = tempfile::tempdir().unwrap();
    chained_log(cut.path(), 5);
    let mut kept = lines(cut.path());
    kept.truncate(3);
    write_lines(cut.path(), &kept);
    assert_eq!(
        check(cut.path(), DEVICE).unwrap(),
        Checked::Broken {
            seq: 3,
            why: Why::HeadIsNotTheSealed
        }
    );
}

/// The head is sealed at each append (the keyring's seal of the device's head follows the log's),
/// and a process that cannot seal makes no claim either way, rather than a false report.
#[test]
fn the_log_head_is_sealed_at_each_append_and_checked_at_start() {
    use crate::secrets::seal;
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    log.chain().unwrap();
    for n in 0..3u64 {
        append(&mut log, "tool.called", n);
        let (seq, chain) = last_chain(dir.path()).unwrap().expect("a chained line");
        assert_eq!(seq, n + 1);
        assert_eq!(
            seal::kept_holds(HEAD_KIND, DEVICE, &head_text(seq, &chain)),
            Some(true),
            "the head after append {seq} is the sealed one"
        );
        if n > 0 {
            let earlier = lines(dir.path());
            let (_, before) = split(earlier[n as usize - 1].as_bytes()).unwrap();
            assert_eq!(
                seal::kept_holds(HEAD_KIND, DEVICE, &head_text(seq - 1, before)),
                Some(false),
                "the earlier head is no longer the sealed one"
            );
        }
    }
    drop(log);
    assert!(matches!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Sealed { seq: 3 }
    ));

    // A process that is not the app cannot verify: no report, whatever the log says.
    seal::set_for_this_test(None);
    assert!(matches!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Unsealed { seq: 3 }
    ));
    seal::set_for_this_test(Some(vec![7; 32]));

    // A seal kept for another device does not vouch for this log's head.
    assert_eq!(
        seal::kept_holds(HEAD_KIND, "another-device", &head_text(3, "00")),
        None
    );
}

/// A chat started with the sandbox off is what chains the log: the line that records it is the
/// first chained one, every line after it is chained to it, and a log that never had such a chat
/// is never chained.
#[test]
fn a_chat_started_with_the_sandbox_off_chains_the_log_from_its_trust_event() {
    use crate::eventlog::{ArgsKey, Recorder};
    let dir = tempfile::tempdir().unwrap();
    let mut host = Recorder::new(
        Log::open(dir.path(), DEVICE).unwrap(),
        ArgsKey::open(dir.path()).unwrap(),
    );
    let body = serde_json::json!({});
    host.write(Some("c"), Some("r"), None, "tool.called", body.clone())
        .unwrap();
    assert!(
        split(lines(dir.path())[0].as_bytes()).is_none(),
        "no chat is off yet: nothing is chained"
    );
    host.write(
        Some("c"),
        Some("r"),
        None,
        "trust.sandbox.off",
        body.clone(),
    )
    .unwrap();
    host.write(Some("c"), Some("r"), None, "tool.called", body)
        .unwrap();
    let written = lines(dir.path());
    assert!(
        split(written[1].as_bytes()).is_some(),
        "the trust line is chained"
    );
    assert!(
        split(written[2].as_bytes()).is_some(),
        "and so is each after it"
    );
    assert!(matches!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Sealed { seq: 3 }
    ));
}

/// A chain that is sealed, then loses the lines before its end (deleted or replaced and
/// recomputed), no longer starts where the app sealed it: the start is the break, not a
/// first line taken on trust (round 2A, B3).
#[test]
fn a_deleted_or_replaced_prefix_is_broken_at_the_start() {
    let dir = tempfile::tempdir().unwrap();
    chained_log(dir.path(), 5);
    let written = lines(dir.path());

    // Lines 1 and 2 deleted: every remaining chain holds, and the head is the sealed one.
    write_lines(dir.path(), &written[2..]);
    assert_eq!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Broken {
            seq: 3,
            why: Why::StartIsNotTheSealed
        }
    );

    // Lines 1 to 4 replaced and every chain recomputed from the first on, ending in the real
    // last line's text: the start differs from the sealed one.
    let mut previous = String::new();
    let mut forged = Vec::new();
    for (at, line) in written.iter().enumerate() {
        let (over, _) = split(line.as_bytes()).unwrap();
        let mut over = String::from_utf8(over).unwrap();
        if at < 4 {
            over = over.replace(&format!("\"n\":{at}"), "\"n\":404");
        }
        let (line, chain) = chained(&previous, over.into_bytes());
        previous = chain;
        forged.push(String::from_utf8(line).unwrap());
    }
    write_lines(dir.path(), &forged);
    assert!(
        check(dir.path(), DEVICE).unwrap().is_broken(),
        "a recomputed chain is told by the sealed start and head"
    );
}

/// A seal is kept for a chain the log no longer holds: the log deleted, or every `chain` key
/// stripped from it, is a break, never "nothing chained" (round 2A, B4).
#[test]
fn a_deleted_log_or_one_with_every_chain_key_stripped_is_broken() {
    let gone = Checked::Broken {
        seq: 0,
        why: Why::ChainIsGone,
    };
    let dir = tempfile::tempdir().unwrap();
    chained_log(dir.path(), 3);
    let written = lines(dir.path());

    let stripped: Vec<String> = written
        .iter()
        .map(|line| String::from_utf8(split(line.as_bytes()).unwrap().0).unwrap())
        .collect();
    write_lines(dir.path(), &stripped);
    assert_eq!(check(dir.path(), DEVICE).unwrap(), gone, "every chain key stripped");
    assert!(gone.said(dir.path()).unwrap().contains("holds none"));

    std::fs::remove_file(dir.path().join(FILE)).unwrap();
    assert_eq!(check(dir.path(), DEVICE).unwrap(), gone, "the log deleted");

    // A log that was never chained, on a machine that sealed one for another device, is
    // still nothing chained.
    let other = tempfile::tempdir().unwrap();
    let mut log = Log::open(other.path(), "another-device").unwrap();
    append(&mut log, "tool.called", 0);
    assert_eq!(check(other.path(), "another-device").unwrap(), Checked::NotChained);
}

/// A writer that opens a log which no longer matches its seals carries on chained but never
/// seals over the mismatch: the forged tail is not adopted as the head, and the check keeps
/// reporting the break (round 2A, B5).
#[test]
fn a_reopen_after_tampering_refuses_to_reseal() {
    use crate::secrets::seal;
    let dir = tempfile::tempdir().unwrap();
    chained_log(dir.path(), 3);
    let written = lines(dir.path());
    // A forged fourth line, chained properly from the third: every chain holds, the seal does
    // not know it, and (as the tail is one past the head) the log reads as stale by one...
    let (_, third) = split(written[2].as_bytes()).unwrap();
    let forged_event = serde_json::json!({
        "v": 1, "device_id": DEVICE, "seq": 4, "ulid": ulid::Ulid::generate().to_string(),
        "chat": "chat", "kind": "tool.called", "body": {"n": 666}
    });
    let (forged, _) = chained(third, serde_json::to_vec(&forged_event).unwrap());
    let mut more = written.clone();
    more.push(String::from_utf8(forged).unwrap());
    // ...so a rewritten line 2 as well makes it a break: the tamper under test.
    more[1] = more[1].replace("\"n\":1", "\"n\":555");
    write_lines(dir.path(), &more);
    assert!(check(dir.path(), DEVICE).unwrap().is_broken());

    let sealed_before = |text: &str| seal::kept_holds(HEAD_KIND, DEVICE, text);
    let (_, head3) = split(written[2].as_bytes()).unwrap();
    assert_eq!(sealed_before(&head_text(3, head3)), Some(true));
    {
        let mut log = Log::open(dir.path(), DEVICE).unwrap();
        assert!(log.is_chained());
        append(&mut log, "tool.called", 9);
    }
    assert_eq!(
        sealed_before(&head_text(3, head3)),
        Some(true),
        "the seal still holds the head it held: nothing was sealed over the mismatch"
    );
    assert!(
        check(dir.path(), DEVICE).unwrap().is_broken(),
        "the break is still reported after the writer carried on"
    );
}

/// A log whose head is one line behind the log's (a crash between an append and its seal) is a
/// warning, not a break; the next append seals it (round 2A, S7).
#[test]
fn a_head_one_line_behind_is_a_warning_and_the_next_append_seals_it() {
    let dir = tempfile::tempdir().unwrap();
    chained_log(dir.path(), 3);
    {
        // The writer cannot seal for one line, as a keyring that failed.
        crate::secrets::seal::set_for_this_test(None);
        let mut log = Log::open(dir.path(), DEVICE).unwrap();
        append(&mut log, "tool.called", 3);
        crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
    }
    let checked = check(dir.path(), DEVICE).unwrap();
    assert_eq!(checked, Checked::Stale { seq: 4 });
    assert!(checked.warned(dir.path()).is_some());
    assert!(!checked.is_broken());
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    append(&mut log, "tool.called", 4);
    assert!(matches!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Sealed { seq: 5 }
    ));
}

/// A chain no seal can be checked against is never an all-clear: it says so (round 2A, B1).
#[test]
fn a_chain_nothing_sealed_is_unsealed_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    crate::secrets::seal::set_for_this_test(None);
    chained_log(dir.path(), 2);
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
    crate::secrets::seal::forget_kept_for_this_test();
    let checked = check(dir.path(), DEVICE).unwrap();
    assert_eq!(checked, Checked::Unsealed { seq: 2 });
    let said = checked.warned(dir.path()).expect("a warning");
    assert!(said.contains("nothing could be checked"), "{said}");
}

/// A chat the project or a template set to level `off` writes no trust event, only a
/// `run.started` that names no sandbox: that line chains the log too (round 2A, S4), and the
/// start-time check reads it (B2).
#[test]
fn a_run_that_starts_with_no_sandbox_chains_the_log() {
    use crate::eventlog::{ArgsKey, Recorder};
    let dir = tempfile::tempdir().unwrap();
    let mut host = Recorder::new(
        Log::open(dir.path(), DEVICE).unwrap(),
        ArgsKey::open(dir.path()).unwrap(),
    );
    let started = |sandbox: &str| serde_json::json!({ "sandbox": sandbox });
    host.write(Some("c"), Some("r"), None, "run.started", started("seatbelt"))
        .unwrap();
    assert!(split(lines(dir.path())[0].as_bytes()).is_none(), "sandboxed: not chained");
    host.write(Some("c"), Some("r2"), None, "run.started", started("none"))
        .unwrap();
    assert!(
        split(lines(dir.path())[1].as_bytes()).is_some(),
        "an unsandboxed run starts the chain"
    );
    assert!(matches!(
        check(dir.path(), DEVICE).unwrap(),
        Checked::Sealed { seq: 2 }
    ));
}

/// Retention deleting the oldest segments is the one way the start moves legitimately: the
/// writer that deleted them anchors the new start, so the log still reads sealed.
#[test]
fn a_prune_by_the_writer_moves_the_sealed_start_and_a_deletion_by_anyone_else_does_not() {
    use crate::eventlog::Retention;
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open_with(
        dir.path(),
        DEVICE,
        Retention {
            segment_bytes: 600,
            keep: std::time::Duration::ZERO,
            ..Retention::DEFAULT
        },
    )
    .unwrap();
    log.chain().unwrap();
    for n in 0..30 {
        append(&mut log, "tool.called", n);
        // Retention keeps what is not older than the clock's millisecond.
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    drop(log);
    assert!(
        segments(dir.path()).unwrap().len() >= 1,
        "segments were sealed"
    );
    assert!(
        matches!(check(dir.path(), DEVICE).unwrap(), Checked::Sealed { .. }),
        "{:?}",
        check(dir.path(), DEVICE)
    );
    let (_, oldest) = segments(dir.path()).unwrap().remove(0);
    std::fs::remove_file(oldest).unwrap();
    assert!(check(dir.path(), DEVICE).unwrap().is_broken());
}
