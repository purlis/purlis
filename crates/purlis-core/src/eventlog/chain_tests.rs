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
