// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::hookwire::{ChatTokens, Decision, ToolCall};

fn call(chat: u32, id: &str) -> ToolCall {
    ToolCall {
        chat,
        tool_hook: "pretooluse".to_owned(),
        tool: Some("Bash".to_owned()),
        call: Some(id.to_owned()),
        args: None,
        decision: Decision::Allow,
        rule: None,
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: None,
    }
}

#[test]
fn each_reason_keeps_the_word_the_event_log_has_always_written() {
    // #983: audit readers match on these words, so the enum that replaced the string constants
    // must say each one byte for byte.
    let pinned = [
        (Why::Unreadable, "unreadable"),
        (Why::NoKey, "no-key"),
        (Why::AnotherChatsKey, "another-chats-key"),
        (Why::Mac, "mac"),
        (Why::NotThisChats, "not-this-chats"),
        (Why::Repeated, "repeated"),
        (Why::Unfinished, "unfinished"),
    ];
    for (why, word) in pinned {
        // Every reason is in the table: a new one fails to compile here until it is pinned.
        match why {
            Why::Unreadable
            | Why::NoKey
            | Why::AnotherChatsKey
            | Why::Mac
            | Why::NotThisChats
            | Why::Repeated
            | Why::Unfinished => {}
        }
        assert_eq!(why.word(), word);
    }
}

/// A host's spool directory with chat `chat` issued a token, as a listener issues one.
fn issued(dir: &Path, chat: u32) -> crate::hookwire::ChatToken {
    ChatTokens::spooling_into(dir.to_path_buf())
        .issue_to_this_process(chat)
        .expect("a token")
}

fn drained(dir: &Path) -> Vec<Drained> {
    let mut all = Vec::new();
    drain(dir, &mut |item| {
        all.push(item);
        Ok(())
    })
    .expect("the spool drains");
    all
}

/// The names in chat `chat`'s spool folder, sorted.
fn names(spool: &Path, chat: u32) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder_for(spool, chat))
        .expect("the chat's folder")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .into_string()
                .expect("a name")
        })
        .collect();
    names.sort();
    names
}

/// The file key `token`'s line number `seq` is in, in chat `chat`'s spool.
fn line_file(spool: &Path, chat: u32, token: &crate::hookwire::ChatToken, seq: u64) -> PathBuf {
    folder_for(spool, chat).join(name_of(&SpoolKey::of(token).id(), seq, Kind::Line))
}

fn tools(items: &[Drained]) -> Vec<(u32, u64, String)> {
    items
        .iter()
        .filter_map(|item| match item {
            Drained::Line {
                chat,
                seq,
                line: Spooled::Tool(call),
            } => Some((*chat, *seq, call.call.clone().unwrap_or_default())),
            _ => None,
        })
        .collect()
}

#[test]
fn a_spooled_line_is_drained_once_in_its_chats_order_with_its_number() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 3);

    for id in ["a", "b", "c"] {
        append(&spool, 3, &token, &call(3, id)).expect("the line is spooled");
    }

    let first = drained(&spool);
    assert_eq!(
        tools(&first),
        [
            (3, 1, "a".to_owned()),
            (3, 2, "b".to_owned()),
            (3, 3, "c".to_owned())
        ]
    );
    assert!(
        first.contains(&Drained::Spool {
            chat: 3,
            from: 1,
            to: 3
        }),
        "{first:?}"
    );
    assert_eq!(names(&spool, 3), Vec::<String>::new(), "its file is gone");
    assert_eq!(drained(&spool), [], "a drained line is never drained again");
}

#[test]
fn a_line_missing_from_the_middle_of_a_spool_is_recorded_as_a_gap() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    for id in ["a", "b", "c", "d"] {
        append(&spool, 4, &token, &call(4, id)).expect("the line is spooled");
    }
    // Something that can write the folder takes out the second and third lines.
    for seq in [2, 3] {
        std::fs::remove_file(line_file(&spool, 4, &token, seq)).expect("the line's file");
    }

    let items = drained(&spool);

    assert_eq!(
        tools(&items),
        [(4, 1, "a".to_owned()), (4, 4, "d".to_owned())]
    );
    assert!(
        items.contains(&Drained::Gap {
            chat: 4,
            from: 2,
            to: 3
        }),
        "{items:?}"
    );
}

#[test]
fn a_line_written_into_another_chats_spool_is_rejected_and_never_read_as_that_chat() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let tokens = ChatTokens::spooling_into(spool.clone());
    let mine = tokens.issue_to_this_process(5).expect("a token");
    let _theirs = tokens.issue_to_this_process(6).expect("a token");

    // Chat 5 holds its own token only. Whatever it writes into chat 6's spool, under its own
    // key or none, names chat 6 and is not chat 6's.
    append(&spool, 6, &mine, &call(6, "forged")).expect("written");
    let forged_line = std::fs::read_to_string(line_file(&spool, 6, &mine, 1)).expect("the line");
    let unkeyed = forged_line.replace("\"mac\":\"", "\"mac\":\"00");
    std::fs::write(line_file(&spool, 6, &mine, 2), unkeyed).expect("written");

    let items = drained(&spool);

    assert_eq!(tools(&items), [], "{items:?}");
    let rejected = items
        .iter()
        .filter(|item| matches!(item, Drained::Rejected { chat: 6, .. }))
        .count();
    assert_eq!(rejected, 2, "{items:?}");
}

#[test]
fn a_line_the_drain_cannot_hand_on_is_left_in_the_spool() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 8);
    append(&spool, 8, &token, &call(8, "a")).expect("the line is spooled");

    let failed = drain(&spool, &mut |_| Err(std::io::Error::other("a full disk")));

    assert!(failed.is_err());
    assert_eq!(
        tools(&drained(&spool)),
        [(8, 1, "a".to_owned())],
        "an event the host could not record is drained at the next start"
    );
}

#[test]
fn a_spool_line_holds_no_token() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 2);
    append(&spool, 2, &token, &call(2, "a")).expect("spooled");

    let text = std::fs::read_to_string(line_file(&spool, 2, &token, 1)).expect("the line");
    let keys = std::fs::read_to_string(spool.join(KEYS)).expect("the keys");
    assert!(!text.contains(token.expose()), "{text}");
    assert!(!keys.contains(token.expose()), "{keys}");
}

#[test]
fn the_spool_and_its_keys_are_private_to_their_owner() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 1);
    append(&spool, 1, &token, &call(1, "a")).expect("spooled");

    for (path, private) in [
        (line_file(&spool, 1, &token, 1), 0o600),
        (spool.join(KEYS), 0o600),
        (folder_for(&spool, 1), 0o700),
    ] {
        let mode = std::fs::metadata(&path)
            .expect("there")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, private, "{}", path.display());
    }
}

#[test]
fn nothing_is_spooled_and_no_key_written_where_the_sandbox_does_not_deny_the_directory() {
    let dir = tempfile::tempdir().expect("a directory");
    // A fallback hook channel's directory, outside any project's `.charter/app/`.
    let elsewhere = dir.path().join("charter-op-0123456789abcdef").join(DIR);
    let token = ChatTokens::default()
        .issue_to_this_process(1)
        .expect("a token");

    let remembered = remember(&elsewhere, 1, &token);
    let spooled = append(&elsewhere, 1, &token, &call(1, "a"));

    assert!(remembered.is_err(), "a key was written outside the denial");
    assert!(spooled.is_err(), "a line was spooled outside the denial");
    assert!(!elsewhere.join(KEYS).exists());
    assert!(!folder_for(&elsewhere, 1).exists());
}

#[test]
fn a_file_that_is_not_a_line_is_rejected_and_neither_the_drain_nor_the_next_hook_stops_on_it() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    // A line torn through a character under the next number, an empty one, and a file no hook
    // names.
    std::fs::write(
        line_file(&spool, 4, &token, 2),
        b"{\"v\":1,\"line\":\"\xe2\x80",
    )
    .expect("written");
    std::fs::write(line_file(&spool, 4, &token, 3), b"").expect("written");
    std::fs::write(folder_for(&spool, 4).join("notes.txt"), b"hello").expect("written");
    append(&spool, 4, &token, &call(4, "b")).expect("the next hook still spools");
    let other = issued(&spool, 5);
    append(&spool, 5, &other, &call(5, "c")).expect("spooled");

    let items = drained(&spool);

    assert_eq!(
        tools(&items),
        [
            (4, 1, "a".to_owned()),
            (4, 4, "b".to_owned()),
            (5, 1, "c".to_owned())
        ]
    );
    let unreadable = Drained::Rejected {
        chat: 4,
        seq: None,
        why: Why::Unreadable,
    };
    assert_eq!(
        items.iter().filter(|item| **item == unreadable).count(),
        3,
        "{items:?}"
    );
    assert!(
        items.contains(&Drained::Gap {
            chat: 4,
            from: 2,
            to: 3
        }),
        "{items:?}"
    );
    assert_eq!(names(&spool, 4), Vec::<String>::new(), "all of it is gone");
}

#[test]
fn a_keys_file_that_does_not_read_rejects_its_lines_as_no_key_and_the_drain_finishes() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 6);
    append(&spool, 6, &token, &call(6, "a")).expect("spooled");
    std::fs::write(spool.join(KEYS), b"{not json").expect("written");

    let items = drained(&spool);

    assert_eq!(
        items,
        [Drained::Rejected {
            chat: 6,
            seq: Some(1),
            why: Why::NoKey
        }]
    );
    assert_eq!(drained(&spool), [], "drained, not stuck");
    issued(&spool, 7);
}

#[test]
fn a_hook_spools_while_another_process_holds_the_chats_spool() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("the first line is spooled");
    // The lock a drain holds on the folder while it reads and hands on, for as long as it
    // takes. A second open is a second holder to flock, as another process is.
    let holder = File::open(folder_for(&spool, 4)).expect("the folder opens");
    holder.lock().expect("the holder takes the lock");

    let spooled = append(&spool, 4, &token, &call(4, "b")).expect("no hook waits for it");

    assert_eq!(spooled, 2);
    drop(holder);
    assert_eq!(
        tools(&drained(&spool)),
        [(4, 1, "a".to_owned()), (4, 2, "b".to_owned())]
    );
}

#[test]
fn a_disk_that_does_not_take_the_line_in_time_is_given_up_on_and_the_line_is_said_lost() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let a_disk_that_hangs = Bounds {
        wait: Duration::from_millis(100),
        a_line_reaches_the_disk: || {
            std::thread::sleep(Duration::from_secs(1));
            Ok(())
        },
        ..Bounds::A_HOOKS
    };

    let started = std::time::Instant::now();
    let refused = append_within(&spool, 4, &token, &call(4, "a"), a_disk_that_hangs)
        .expect_err("the hook does not wait for it");
    let waited = started.elapsed();

    assert_eq!(refused.kind(), io::ErrorKind::TimedOut, "{refused}");
    assert!(refused.to_string().contains("within 100 ms"), "{refused}");
    assert!(
        waited >= Duration::from_millis(100) && waited < Duration::from_millis(900),
        "waited {waited:?}"
    );
}

#[test]
fn a_disk_that_refuses_the_line_says_so_and_leaves_nothing_behind() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let a_full_disk = Bounds {
        a_line_reaches_the_disk: || Err(io::Error::other("no space left on the device")),
        ..Bounds::A_HOOKS
    };

    let refused = append_within(&spool, 4, &token, &call(4, "a"), a_full_disk)
        .expect_err("the line is not spooled");

    assert!(refused.to_string().contains("no space left"), "{refused}");
    assert_eq!(
        names(&spool, 4),
        Vec::<String>::new(),
        "no number is held for a line that was never kept"
    );
    append(&spool, 4, &token, &call(4, "b")).expect("a disk with room takes the next line");
    assert_eq!(tools(&drained(&spool)), [(4, 1, "b".to_owned())]);
}

#[test]
fn a_spool_past_what_a_hook_lists_takes_no_more_and_the_line_is_said_lost() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let three_lines = Bounds {
        lines: 3,
        ..Bounds::A_HOOKS
    };
    for id in ["a", "b", "c"] {
        append_within(&spool, 4, &token, &call(4, id), three_lines).expect("there is room");
    }

    let refused = append_within(&spool, 4, &token, &call(4, "d"), three_lines)
        .expect_err("the spool is full");

    assert_eq!(refused.kind(), io::ErrorKind::InvalidData, "{refused}");
    assert!(
        refused.to_string().contains("already holds 3 lines"),
        "{refused}"
    );
    assert_eq!(names(&spool, 4).len(), 3, "nothing was written for it");
    assert_eq!(tools(&drained(&spool)).len(), 3);
}

#[test]
fn a_number_a_dead_hook_took_is_a_gap_and_its_file_goes_once_it_is_old() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    // A hook that took the second number and died before its line was whole.
    let part = folder_for(&spool, 4).join(name_of(&SpoolKey::of(&token).id(), 2, Kind::Part));
    std::fs::write(&part, b"{\"v\":1,\"se").expect("written");
    let third = append(&spool, 4, &token, &call(4, "c")).expect("the next hook still spools");

    let items = drained(&spool);

    assert_eq!(third, 3, "the dead hook's number is not given twice");
    assert_eq!(
        tools(&items),
        [(4, 1, "a".to_owned()), (4, 3, "c".to_owned())]
    );
    assert!(
        items.contains(&Drained::Gap {
            chat: 4,
            from: 2,
            to: 2
        }),
        "{items:?}"
    );
    let unfinished = Drained::Rejected {
        chat: 4,
        seq: Some(2),
        why: Why::Unfinished,
    };
    assert!(items.contains(&unfinished), "{items:?}");
    assert!(part.exists(), "a hook may still be writing it");

    // Written long ago, or at a time that has not come: either way no hook is writing it.
    let now = std::time::SystemTime::now();
    for when in [
        now - 2 * A_PART_IS_A_DEAD_HOOKS_AFTER,
        now + 2 * A_PART_IS_A_DEAD_HOOKS_AFTER,
    ] {
        std::fs::write(&part, b"").expect("written");
        File::options()
            .write(true)
            .open(&part)
            .and_then(|file| file.set_modified(when))
            .expect("its time is set");
        assert_eq!(
            drained(&spool),
            std::slice::from_ref(&unfinished),
            "said, as it is removed"
        );
        assert!(!part.exists(), "no hook lives that long");
    }
}

#[test]
fn a_part_file_under_the_last_number_there_is_stops_its_keys_hooks_and_the_drain_says_so() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    let part =
        folder_for(&spool, 4).join(name_of(&SpoolKey::of(&token).id(), u64::MAX, Kind::Part));
    std::fs::write(&part, b"").expect("planted");

    let refused = append(&spool, 4, &token, &call(4, "b")).expect_err("no number is left");

    assert!(
        refused.to_string().contains("used every number"),
        "{refused}"
    );
    let items = drained(&spool);
    assert_eq!(tools(&items), [(4, 1, "a".to_owned())]);
    assert!(
        items.contains(&Drained::Rejected {
            chat: 4,
            seq: Some(u64::MAX),
            why: Why::Unfinished,
        }),
        "{items:?}"
    );
}

/// What the drain does with a line that was still being written while it ran: its number is a
/// gap at that drain, and the host keeps the key and what it drained under it, so the next
/// drain hands the line on, once (V99i).
#[test]
fn a_line_in_flight_while_a_drain_runs_is_a_gap_then_and_handed_on_at_the_next() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    // A hook has taken the second number and is still writing its line.
    let part = folder_for(&spool, 4).join(name_of(&SpoolKey::of(&token).id(), 2, Kind::Part));
    std::fs::write(&part, b"").expect("its number is taken");
    append(&spool, 4, &token, &call(4, "c")).expect("spooled");

    let first = drained(&spool);
    // The hook finishes after the drain has.
    std::fs::write(&part, a_spool_line(4, &token, 2, "b")).expect("written");
    std::fs::rename(&part, line_file(&spool, 4, &token, 2)).expect("named");
    let second = drained(&spool);
    // And somebody puts its file back once it is drained.
    std::fs::write(
        line_file(&spool, 4, &token, 2),
        a_spool_line(4, &token, 2, "b"),
    )
    .expect("put back");
    let third = drained(&spool);

    assert_eq!(
        tools(&first),
        [(4, 1, "a".to_owned()), (4, 3, "c".to_owned())]
    );
    for said in [
        Drained::Gap {
            chat: 4,
            from: 2,
            to: 2,
        },
        Drained::Rejected {
            chat: 4,
            seq: Some(2),
            why: Why::Unfinished,
        },
    ] {
        assert!(first.contains(&said), "{said:?} is not in {first:?}");
    }
    assert_eq!(tools(&second), [(4, 2, "b".to_owned())], "{second:?}");
    assert_eq!(second.len(), 2, "the line and its sequence: {second:?}");
    assert_eq!(
        third,
        [Drained::Rejected {
            chat: 4,
            seq: Some(2),
            why: Why::Repeated,
        }]
    );
}

/// The same line when the drain it was in flight for stopped before its end: handed on at the
/// next, with no gap said for the numbers the stopped drain had already taken.
#[test]
fn a_line_in_flight_while_a_drain_that_stops_runs_is_handed_on_at_the_next_with_no_gap() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let other = issued(&spool, 8);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    let part = folder_for(&spool, 4).join(name_of(&SpoolKey::of(&token).id(), 2, Kind::Part));
    std::fs::write(&part, b"").expect("its number is taken");
    append(&spool, 8, &other, &call(8, "x")).expect("spooled");

    // The host records chat 4's line and cannot record chat 8's.
    let stopped = drain(&spool, &mut |item| match item.chat() {
        8 => Err(io::Error::other("a full disk")),
        _ => Ok(()),
    });
    std::fs::write(&part, a_spool_line(4, &token, 2, "b")).expect("written");
    std::fs::rename(&part, line_file(&spool, 4, &token, 2)).expect("named");
    let next = drained(&spool);

    assert!(stopped.is_err());
    assert_eq!(
        tools(&next),
        [(4, 2, "b".to_owned()), (8, 1, "x".to_owned())]
    );
    assert!(
        !next.iter().any(|item| matches!(item, Drained::Gap { .. })),
        "line 1 was taken by the drain that stopped, and is no gap: {next:?}"
    );
}

/// A line put back after a drain that stopped part-way had taken it is not taken again: what a
/// drain handed on for one chat is kept even when it could not finish the others.
#[test]
fn a_line_put_back_after_a_drain_that_stopped_part_way_is_rejected_as_repeated() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 7);
    let other = issued(&spool, 8);
    append(&spool, 7, &token, &call(7, "a")).expect("spooled");
    append(&spool, 8, &other, &call(8, "x")).expect("spooled");
    let saved = std::fs::read(line_file(&spool, 7, &token, 1)).expect("the line");

    let stopped = drain(&spool, &mut |item| match item.chat() {
        8 => Err(io::Error::other("a full disk")),
        _ => Ok(()),
    });
    std::fs::write(line_file(&spool, 7, &token, 1), saved).expect("put back");
    let next = drained(&spool);

    assert!(stopped.is_err());
    assert_eq!(tools(&next), [(8, 1, "x".to_owned())], "{next:?}");
    assert!(
        next.contains(&Drained::Rejected {
            chat: 7,
            seq: Some(1),
            why: Why::Repeated,
        }),
        "{next:?}"
    );
}

/// A hook that outlives a drain, as one of a chat that is still open does: its next line takes
/// the number after the last one drained, and the next drain hands it on with no gap.
#[test]
fn a_hook_that_outlives_a_drain_numbers_on_from_it_and_its_line_is_handed_on() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    for id in ["a", "b"] {
        append(&spool, 4, &token, &call(4, id)).expect("spooled");
    }
    assert_eq!(tools(&drained(&spool)).len(), 2);

    let third = append(&spool, 4, &token, &call(4, "c")).expect("spooled");
    let next = drained(&spool);

    assert_eq!(third, 3, "the folder is empty, and the number goes on");
    assert_eq!(
        next,
        [
            Drained::Line {
                chat: 4,
                seq: 3,
                line: Spooled::Tool(call(4, "c")),
            },
            Drained::Spool {
                chat: 4,
                from: 3,
                to: 3,
            },
        ]
    );
}

/// Only a line that checks moves what is kept. A file planted under a high number, which has
/// no MAC of its own, is rejected and leaves the key's real lines as they were: the next one
/// is handed on, not `repeated`.
#[test]
fn a_planted_file_under_a_high_number_does_not_make_real_lines_read_as_repeated() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    let forged = a_spool_line(4, &token, 900, "planted").replace("\"mac\":\"", "\"mac\":\"00");
    std::fs::write(line_file(&spool, 4, &token, 900), forged).expect("planted");

    let first = drained(&spool);
    let second = append(&spool, 4, &token, &call(4, "b")).expect("spooled");
    let next = drained(&spool);

    assert_eq!(tools(&first), [(4, 1, "a".to_owned())], "{first:?}");
    assert!(
        first.contains(&Drained::Rejected {
            chat: 4,
            seq: Some(900),
            why: Why::Mac,
        }),
        "{first:?}"
    );
    assert_eq!(second, 2, "the planted number was never drained");
    assert_eq!(tools(&next), [(4, 2, "b".to_owned())], "{next:?}");
    assert_eq!(next.len(), 2, "no gap and nothing rejected: {next:?}");
}

#[test]
fn a_line_copied_from_the_folder_into_an_older_builds_file_is_handed_on_once() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let mut copied = Vec::new();
    for (seq, id) in [(1, "a"), (2, "b")] {
        append(&spool, 4, &token, &call(4, id)).expect("spooled");
        copied.extend(std::fs::read(line_file(&spool, 4, &token, seq)).expect("the line"));
    }
    // Something that can write the spool copies both lines, MAC and all, into the file a
    // build before #983 wrote. It needs no token for that.
    std::fs::write(file_for(&spool, 4), copied).expect("the copy");

    let items = drained(&spool);

    assert_eq!(
        tools(&items),
        [(4, 1, "a".to_owned()), (4, 2, "b".to_owned())],
        "{items:?}"
    );
    for seq in [1, 2] {
        let repeated = Drained::Rejected {
            chat: 4,
            seq: Some(seq),
            why: Why::Repeated,
        };
        assert!(
            items.contains(&repeated),
            "{repeated:?} is not in {items:?}"
        );
    }
}

#[test]
fn a_line_put_back_after_its_drain_is_rejected_as_repeated() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    let saved = std::fs::read(line_file(&spool, 4, &token, 1)).expect("the line");
    assert_eq!(tools(&drained(&spool)), [(4, 1, "a".to_owned())]);

    std::fs::write(line_file(&spool, 4, &token, 1), saved).expect("put back");

    assert_eq!(
        drained(&spool),
        [Drained::Rejected {
            chat: 4,
            seq: Some(1),
            why: Why::Repeated,
        }]
    );
}

#[test]
fn a_line_file_is_one_line_under_its_own_key_and_number_or_it_is_rejected() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    for id in ["a", "b", "c"] {
        append(&spool, 4, &token, &call(4, id)).expect("spooled");
    }
    // The second line under a number that is not its own, the third under a key that is not
    // its own, and two lines that check in one file.
    std::fs::rename(
        line_file(&spool, 4, &token, 2),
        line_file(&spool, 4, &token, 9),
    )
    .expect("renamed");
    std::fs::rename(
        line_file(&spool, 4, &token, 3),
        folder_for(&spool, 4).join("0000000000000000.3.json"),
    )
    .expect("renamed");
    std::fs::write(
        line_file(&spool, 4, &token, 4),
        a_spool_line(4, &token, 4, "d") + &a_spool_line(4, &token, 5, "e"),
    )
    .expect("written");

    let items = drained(&spool);

    assert_eq!(tools(&items), [(4, 1, "a".to_owned())], "{items:?}");
    let unreadable = Drained::Rejected {
        chat: 4,
        seq: None,
        why: Why::Unreadable,
    };
    assert_eq!(
        items.iter().filter(|item| **item == unreadable).count(),
        3,
        "{items:?}"
    );
    assert_eq!(names(&spool, 4), Vec::<String>::new(), "all of it is gone");
}

#[test]
fn a_link_a_directory_or_a_pipe_under_a_lines_name_is_rejected_and_never_followed() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    // A link to a line that would check, outside the folder; a directory; a pipe nobody
    // writes to.
    let outside = dir.path().join("outside.json");
    std::fs::write(&outside, a_spool_line(4, &token, 2, "linked")).expect("its target");
    std::os::unix::fs::symlink(&outside, line_file(&spool, 4, &token, 2)).expect("a link");
    std::fs::create_dir(line_file(&spool, 4, &token, 3)).expect("a directory");
    let made = crate::forklock::status(
        std::process::Command::new("mkfifo").arg(line_file(&spool, 4, &token, 4)),
    )
    .expect("mkfifo runs");
    assert!(made.success(), "mkfifo");
    let next = append(&spool, 4, &token, &call(4, "b")).expect("the next hook still spools");

    let items = drained(&spool);

    assert_eq!(
        next, 5,
        "a name that is taken is stepped past, never written through"
    );
    assert_eq!(
        tools(&items),
        [(4, 1, "a".to_owned()), (4, 5, "b".to_owned())]
    );
    let unreadable = Drained::Rejected {
        chat: 4,
        seq: None,
        why: Why::Unreadable,
    };
    assert_eq!(
        items.iter().filter(|item| **item == unreadable).count(),
        3,
        "{items:?}"
    );
    assert!(outside.exists(), "the link's target is not touched");
    assert_eq!(
        names(&spool, 4),
        [name_of(&SpoolKey::of(&token).id(), 3, Kind::Line)],
        "the link and the pipe are gone; a directory is left for its owner"
    );
}

#[test]
fn the_cap_counts_every_name_in_the_folder_whatever_it_is() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let three_names = Bounds {
        lines: 3,
        ..Bounds::A_HOOKS
    };
    append_within(&spool, 4, &token, &call(4, "a"), three_names).expect("there is room");
    // A number another key's hook holds, and a file no hook names.
    std::fs::write(folder_for(&spool, 4).join("0000000000000000.7.part"), b"").expect("written");
    std::fs::write(folder_for(&spool, 4).join("notes.txt"), b"").expect("written");

    let refused = append_within(&spool, 4, &token, &call(4, "b"), three_names)
        .expect_err("the spool is full");

    assert_eq!(refused.kind(), io::ErrorKind::InvalidData, "{refused}");
    assert_eq!(names(&spool, 4).len(), 3, "nothing was written for it");
}

#[test]
fn a_write_that_stops_without_answering_says_so_and_not_that_the_disk_was_slow() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let a_write_that_panics = Bounds {
        a_line_reaches_the_disk: || panic!("this test's own, to stop the write"),
        ..Bounds::A_HOOKS
    };

    let refused = append_within(&spool, 4, &token, &call(4, "a"), a_write_that_panics)
        .expect_err("the line is not spooled");

    assert_ne!(refused.kind(), io::ErrorKind::TimedOut, "{refused}");
    assert!(refused.to_string().contains("stopped"), "{refused}");
}

/// The object a spool line is, with its newline: what a line's file holds, and what a build
/// before #983 appended to the chat's one file.
fn a_spool_line(chat: u32, token: &crate::hookwire::ChatToken, seq: u64, id: &str) -> String {
    let key = SpoolKey::of(token);
    let line = serde_json::to_string(&call(chat, id)).expect("a line");
    let mac = key.sign(chat, seq, &key.id(), &line);
    let mut text = serde_json::to_string(&LineFile {
        v: VERSION,
        seq,
        key: key.id(),
        line,
        mac,
    })
    .expect("a spool line");
    text.push('\n');
    text
}

/// Rewrites `spool`'s `keys.json` as a build before V99i wrote it: version 1, and each key
/// with its id, its chat and the key alone. What this build makes of that is a key a build
/// before issued, whose hooks may have written the chat's one file.
fn as_a_build_before_wrote_its_keys(spool: &Path) {
    let mut keys = keys_of(spool);
    keys["v"] = 1.into();
    for held in keys["keys"].as_array_mut().expect("keys") {
        let held = held.as_object_mut().expect("a key");
        held.retain(|field, _| matches!(field.as_str(), "id" | "chat" | "key"));
    }
    std::fs::write(spool.join(KEYS), keys.to_string()).expect("the old shape");
}

#[test]
fn a_spool_file_an_older_build_left_is_drained_before_the_chats_folder_and_emptied() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let other = issued(&spool, 6);
    as_a_build_before_wrote_its_keys(&spool);
    // Chat 4 has both: an older hook's file, with a line a crash tore, and this build's lines.
    let old = file_for(&spool, 4);
    let mut text = a_spool_line(4, &token, 1, "old-a");
    text.push_str(&a_spool_line(4, &token, 3, "old-c"));
    text.push_str("{\"v\":1,\"seq\":4,\"ke");
    std::fs::write(&old, text).expect("the old spool");
    append(&spool, 4, &token, &call(4, "new-a")).expect("spooled");
    // Chat 6 has the old file alone.
    std::fs::write(file_for(&spool, 6), a_spool_line(6, &other, 1, "old-only"))
        .expect("the old spool");

    let items = drained(&spool);

    assert_eq!(
        tools(&items),
        [
            (4, 1, "old-a".to_owned()),
            (4, 3, "old-c".to_owned()),
            (4, 1, "new-a".to_owned()),
            (6, 1, "old-only".to_owned())
        ]
    );
    for found in [
        Drained::Gap {
            chat: 4,
            from: 2,
            to: 2,
        },
        Drained::Rejected {
            chat: 4,
            seq: None,
            why: Why::Unreadable,
        },
        Drained::Spool {
            chat: 4,
            from: 1,
            to: 3,
        },
        Drained::Spool {
            chat: 4,
            from: 1,
            to: 1,
        },
    ] {
        assert!(items.contains(&found), "{found:?} is not in {items:?}");
    }
    assert_eq!(
        std::fs::read(&old).expect("still there, for a hook holding it open"),
        b"",
        "emptied"
    );
    assert!(
        !folder_for(&spool, 6).exists(),
        "nothing is made to read it"
    );
    assert_eq!(drained(&spool), [], "drained once");
}

#[test]
fn two_keys_lines_are_handed_on_in_the_order_the_host_issued_the_keys() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let now = std::time::SystemTime::now();
    let mut started = Vec::new();
    // Chat 4 started three times, each start with a token of its own, and spooled under each.
    // The files' times say the opposite order, and are not what is read.
    for (start, ago) in [(0, 10), (1, 20), (2, 30)] {
        let token = issued(&spool, 4);
        for line in 1..=2 {
            let id = format!("start-{start}-{line}");
            let seq = append(&spool, 4, &token, &call(4, &id)).expect("spooled");
            File::options()
                .write(true)
                .open(line_file(&spool, 4, &token, seq))
                .and_then(|file| file.set_modified(now - Duration::from_secs(ago)))
                .expect("its time is set");
            started.push((4, seq, id));
        }
    }

    assert_eq!(tools(&drained(&spool)), started);
}

/// What `append` answers for chat `chat` in `spool`, or `None` if it has not answered in two
/// seconds: a hook must answer its harness whatever sits at the spool path.
fn appended_within_two_seconds(
    spool: &Path,
    chat: u32,
    token: crate::hookwire::ChatToken,
) -> Option<io::Result<u64>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let spool = spool.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(append(&spool, chat, &token, &call(chat, "a")));
    });
    rx.recv_timeout(Duration::from_secs(2)).ok()
}

#[test]
fn a_spool_path_that_is_not_a_folder_is_refused_at_once_and_the_line_is_lost() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 5);
    // A named pipe, where nothing ever writes: a blocking open or read of it never returns.
    let made =
        crate::forklock::status(std::process::Command::new("mkfifo").arg(folder_for(&spool, 5)))
            .expect("mkfifo runs");
    assert!(made.success(), "mkfifo");

    let answered = appended_within_two_seconds(&spool, 5, token).expect("the hook answered");
    let refused = answered.expect_err("nothing is spooled into a pipe");
    assert_eq!(refused.kind(), io::ErrorKind::InvalidInput, "{refused}");
}

#[test]
fn a_spool_that_is_not_what_it_should_be_is_reported_and_the_drain_reads_the_others() {
    /// What sits at the spool path, and how it is put there.
    type Planted<'a> = (&'a str, &'a dyn Fn(&Path));
    let pipe: Planted<'_> = ("a pipe", &|at: &Path| {
        let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(at))
            .expect("mkfifo runs");
        assert!(made.success(), "mkfifo");
    });
    let directory: Planted<'_> = ("a directory", &|at: &Path| {
        std::fs::create_dir(at).expect("mkdir")
    });
    let file: Planted<'_> = ("a file", &|at: &Path| {
        std::fs::write(at, b"").expect("a file")
    });
    let dangling: Planted<'_> = ("a dangling link", &|at: &Path| {
        std::os::unix::fs::symlink(at.with_extension("gone"), at).expect("a link");
    });
    let to_a_file: Planted<'_> = ("a link to a file", &|at: &Path| {
        let to = at.with_extension("real");
        std::fs::write(&to, b"").expect("its target");
        std::os::unix::fs::symlink(&to, at).expect("a link");
    });
    let to_a_folder: Planted<'_> = ("a link to a folder", &|at: &Path| {
        let to = at.with_extension("real");
        std::fs::create_dir(&to).expect("its target");
        std::os::unix::fs::symlink(&to, at).expect("a link");
    });
    /// Where chat 5's spool is: this build's folder, or an older build's file.
    type Spool = fn(&Path, u32) -> PathBuf;
    let planted: [(Spool, &[Planted<'_>]); 2] = [
        (file_for, &[pipe, directory, dangling, to_a_file]),
        (folder_for, &[pipe, file, dangling, to_a_file, to_a_folder]),
    ];
    for (at, plants) in planted {
        for (what, plant) in plants {
            let dir = tempfile::tempdir().expect("a directory");
            let spool = dir.path().join(".charter/app").join(DIR);
            let token = issued(&spool, 7);
            issued(&spool, 5);
            append(&spool, 7, &token, &call(7, "a")).expect("the line is spooled");
            let at = at(&spool, 5);
            plant(&at);

            let all = drained(&spool);
            assert!(
                all.contains(&Drained::Rejected {
                    chat: 5,
                    seq: None,
                    why: Why::Unreadable
                }),
                "{what} at {}: {all:?}",
                at.display()
            );
            assert_eq!(tools(&all), [(7, 1, "a".to_owned())], "{what}");
        }
    }
}

/// How many hooks are inside [`on_a_slow_disk`]'s disk now.
///
/// **One caller only**: [`sixteen_hooks_spooling_two_thousand_lines_for_one_chat_on_a_slow_disk_lose_none`].
/// `cargo test` runs a binary's tests on threads of one process, so a second test on this disk
/// would add its own hooks to the count, and two tests' hooks overlapping would hide one test's
/// hooks queuing behind each other.
static ON_THE_SLOW_DISK_NOW: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// The most hooks that were ever inside [`on_a_slow_disk`]'s disk at once. One caller only, as
/// [`ON_THE_SLOW_DISK_NOW`] says.
static ON_THE_SLOW_DISK_AT_MOST: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// A hook's bounds on a disk that takes 20 ms to make a line durable, as a loaded machine's
/// does, and that counts the hooks inside it at once ([`ON_THE_SLOW_DISK_NOW`],
/// [`ON_THE_SLOW_DISK_AT_MOST`]).
///
/// **The wait is a hang's, not a hook's** (#2410). What #983 holds is that no hook waits for
/// another, and that is the count's to show: hooks that queued for each other would never be
/// on the disk two at a time. A hook's own second ([`A_LINE_IS_SPOOLED_WITHIN`]) is a wall
/// clock's, and a loaded runner's disk outlasted it with no hook waiting on another: six lines
/// of sixteen hooks, all at one moment, while every other line was spooled. Giving up on a disk
/// in time is held by
/// `a_disk_that_does_not_take_the_line_in_time_is_given_up_on_and_the_line_is_said_lost`, and
/// a hook's wait being that second by `a_hooks_wait_for_its_line_is_the_spool_bound`.
///
/// **What the count shows, and what it does not**: hooks inside this injected 20 ms at once,
/// which a lock taken anywhere around the write would prevent. It does not show that their
/// real `fsync`s, after it, run at the same time: a lock taken only around those would pass.
fn on_a_slow_disk() -> Bounds {
    Bounds {
        wait: Duration::from_secs(60),
        a_line_reaches_the_disk: || {
            use std::sync::atomic::Ordering::SeqCst;
            let inside = ON_THE_SLOW_DISK_NOW.fetch_add(1, SeqCst) + 1;
            ON_THE_SLOW_DISK_AT_MOST.fetch_max(inside, SeqCst);
            std::thread::sleep(Duration::from_millis(20));
            ON_THE_SLOW_DISK_NOW.fetch_sub(1, SeqCst);
            Ok(())
        },
        ..Bounds::A_HOOKS
    }
}

/// #2410: a hook gives its line [`A_LINE_IS_SPOOLED_WITHIN`], a second, and no longer. The
/// sixteen-hook test waits as long as a hang, so this pins the hook's own bound.
#[test]
fn a_hooks_wait_for_its_line_is_the_spool_bound() {
    assert_eq!(Bounds::A_HOOKS.wait, Duration::from_secs(1));
    assert_eq!(A_LINE_IS_SPOOLED_WITHIN, Duration::from_secs(1));
}

/// Issue 983: hooks of one chat that spool at once never cost each other a line. Sixteen
/// writers, two thousand lines, and a disk slow enough that a writer queued behind the others'
/// syncs would run out of any wait a hook can afford. That they never queue is shown by the
/// event, hooks on the disk together, never by a wall clock (#2410).
#[test]
fn sixteen_hooks_spooling_two_thousand_lines_for_one_chat_on_a_slow_disk_lose_none() {
    const WRITERS: usize = 16;
    const EACH: usize = 125;
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 9);
    let lost = std::sync::Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for writer in 0..WRITERS {
            let (spool, token, lost) = (&spool, &token, &lost);
            scope.spawn(move || {
                for n in 0..EACH {
                    let id = format!("{writer:02}-{n:03}");
                    if let Err(why) =
                        append_within(spool, 9, token, &call(9, &id), on_a_slow_disk())
                    {
                        lost.lock().unwrap().push(format!("{id}: {why}"));
                    }
                }
            });
        }
    });

    let most = ON_THE_SLOW_DISK_AT_MOST.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        most > 1,
        "no two hooks were ever on the disk at once, so each waited for the others' writes"
    );
    let lost = lost.into_inner().unwrap();
    assert!(
        lost.is_empty(),
        "{} of {} lines were not spooled, the first of them: {:#?}",
        lost.len(),
        WRITERS * EACH,
        &lost[..lost.len().min(5)]
    );
    let items = drained(&spool);
    let lines = tools(&items);
    assert_eq!(
        lines.iter().map(|(_, seq, _)| *seq).collect::<Vec<_>>(),
        (1..=(WRITERS * EACH) as u64).collect::<Vec<_>>(),
        "every line has a number of its own, and none is missing"
    );
    assert_eq!(
        items.len(),
        WRITERS * EACH + 1,
        "nothing but the lines and their sequence: no gap, nothing rejected"
    );
    // Each writer's lines are in the order it spooled them.
    for writer in 0..WRITERS {
        let mine: Vec<&String> = lines
            .iter()
            .map(|(_, _, id)| id)
            .filter(|id| id.starts_with(&format!("{writer:02}-")))
            .collect();
        let mut sorted = mine.clone();
        sorted.sort();
        assert_eq!(mine.len(), EACH, "writer {writer}");
        assert_eq!(mine, sorted, "writer {writer}'s lines are out of its order");
    }
}

/// A drain that runs while hooks spool, as a host's one drain at a project's open would if
/// hooks of the last run were still about. It hands on each line it read, once, and removes
/// those files and no other. Every line a hook finishes after that is handed on by the next
/// drain, once, and a number the first drain called a gap is one of those lines: none is lost
/// and none is handed on twice (V99i).
#[test]
fn a_drain_racing_hooks_and_the_one_after_hand_on_every_line_once() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    const WRITERS: usize = 8;
    const EACH: usize = 100;
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 9);
    let spooled = AtomicUsize::new(0);

    let racing = std::thread::scope(|scope| {
        for writer in 0..WRITERS {
            let (spool, token, spooled) = (&spool, &token, &spooled);
            scope.spawn(move || {
                for n in 0..EACH {
                    let id = format!("{writer:02}-{n:03}");
                    append(spool, 9, token, &call(9, &id)).expect("the line is spooled");
                    spooled.fetch_add(1, Ordering::SeqCst);
                }
            });
        }
        // The drains start once the hooks are well under way, and go on while they spool.
        while spooled.load(Ordering::SeqCst) < WRITERS * EACH / 4 {
            std::thread::yield_now();
        }
        let mut racing = Vec::new();
        while spooled.load(Ordering::SeqCst) < WRITERS * EACH * 3 / 4 {
            racing.extend(drained(&spool));
        }
        racing
    });
    let after = drained(&spool);

    let handed_on: Vec<(u64, String)> = tools(&racing)
        .into_iter()
        .chain(tools(&after))
        .map(|(_, seq, id)| (seq, id))
        .collect();
    let mut ids: Vec<&String> = handed_on.iter().map(|(_, id)| id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), handed_on.len(), "a line was handed on twice");
    assert_eq!(ids.len(), WRITERS * EACH, "a line was never handed on");
    let mut numbers: Vec<u64> = handed_on.iter().map(|(seq, _)| *seq).collect();
    numbers.sort_unstable();
    assert_eq!(
        numbers,
        (1..=(WRITERS * EACH) as u64).collect::<Vec<_>>(),
        "every number once, so every gap a drain said was a line in flight"
    );
    for item in racing.iter().chain(&after) {
        match item {
            Drained::Line { .. } | Drained::Gap { .. } | Drained::Spool { .. } => {}
            // A number a hook held while a drain looked.
            Drained::Rejected {
                why: Why::Unfinished,
                ..
            } => {}
            other => panic!("a drain found {other:?}"),
        }
    }
    assert!(
        !after.iter().any(|item| matches!(item, Drained::Gap { .. })),
        "nothing is in flight any more: {after:?}"
    );
    assert_eq!(names(&spool, 9), Vec::<String>::new(), "nothing is left");
    assert_eq!(drained(&spool), [], "and nothing is handed on again");
}

/// The `keys.json` of `spool`, as JSON.
fn keys_of(spool: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(spool.join(KEYS)).expect("the keys")).expect("JSON")
}

/// The chat and the id of each key `keys.json` holds, in its order.
fn kept(spool: &Path) -> Vec<(u64, String)> {
    keys_of(spool)["keys"]
        .as_array()
        .expect("keys")
        .iter()
        .map(|held| {
            (
                held["chat"].as_u64().expect("a chat"),
                held["id"].as_str().expect("an id").to_owned(),
            )
        })
        .collect()
}

#[test]
fn a_keys_file_in_the_shape_before_is_read_and_written_in_this_one_with_no_key_dropped() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let key = SpoolKey::of(&token);
    // What a build before wrote: version 1, and a key with nothing said of what was drained.
    let before = serde_json::json!({
        "v": 1,
        "keys": [{ "id": key.id(), "chat": 4, "key": crate::extension::hex(&key.0) }],
    });
    std::fs::write(spool.join(KEYS), before.to_string()).expect("the old shape");
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");

    let items = drained(&spool);

    assert_eq!(tools(&items), [(4, 1, "a".to_owned())], "{items:?}");
    let now = keys_of(&spool);
    assert_eq!(now["v"], 2, "{now}");
    assert_eq!(now["keys"][0]["id"], key.id().as_str(), "{now}");
    assert_eq!(now["keys"][0]["drained"], 1, "{now}");
    assert_eq!(
        now["keys"][0]["file"], 0,
        "marked as a key a build before issued, with nothing drained from its file: {now}"
    );
    issued(&spool, 5);
    assert!(
        keys_of(&spool)["keys"][1].get("file").is_none(),
        "a key this build issues has no file of lines"
    );
}

#[test]
fn a_keys_file_a_newer_build_wrote_is_refused_and_left_as_it_is() {
    // Version 3, and versions written in ways no build of this one writes: none is 1 or 2.
    for version in ["3", "3.0", "\"3\"", "4294967298", "0", "null"] {
        let dir = tempfile::tempdir().expect("a directory");
        let spool = dir.path().join(".charter/app").join(DIR);
        let token = issued(&spool, 4);
        append(&spool, 4, &token, &call(4, "a")).expect("spooled");
        let newer =
            format!(r#"{{"v":{version},"keys":[],"something":"this build does not know"}}"#);
        std::fs::write(spool.join(KEYS), &newer).expect("a newer shape");

        let refused = drain(&spool, &mut |_| Ok(())).expect_err("nothing is drained");
        let not_added = remember(&spool, 5, &token).expect_err("no key is added");

        for refused in [refused, not_added] {
            assert!(refused.to_string().contains("a newer purlis"), "{refused}");
            assert!(
                refused.to_string().contains(&format!("version {version}")),
                "{refused}"
            );
        }
        assert_eq!(
            std::fs::read_to_string(spool.join(KEYS)).expect("the keys"),
            newer,
            "version {version}"
        );
        assert_eq!(names(&spool, 4).len(), 1, "the line waits for that build");
        append(&spool, 4, &token, &call(4, "b")).expect("a hook still spools beside it");
    }
}

/// The bound on `keys.json`: after a drain, one key per chat, the last one it was issued.
#[test]
fn a_drain_keeps_each_chats_newest_key_and_drops_the_ones_issued_before_it() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    // Chat 4 started three times, and spooled under its first key and its last.
    let first = issued(&spool, 4);
    issued(&spool, 4);
    let last = issued(&spool, 4);
    let other = issued(&spool, 5);
    append(&spool, 4, &first, &call(4, "a")).expect("spooled");
    append(&spool, 4, &last, &call(4, "b")).expect("spooled");
    assert_eq!(kept(&spool).len(), 4, "one per token issued, until a drain");

    let items = drained(&spool);

    assert_eq!(
        tools(&items),
        [(4, 1, "a".to_owned()), (4, 1, "b".to_owned())],
        "a key issued before is drained before it is dropped"
    );
    assert_eq!(
        kept(&spool),
        [
            (4, SpoolKey::of(&last).id()),
            (5, SpoolKey::of(&other).id())
        ]
    );
}

#[test]
fn a_key_remembered_twice_is_held_once_and_keeps_what_was_drained_under_it() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    let saved = std::fs::read(line_file(&spool, 4, &token, 1)).expect("the line");
    assert_eq!(tools(&drained(&spool)).len(), 1);

    remember(&spool, 4, &token).expect("remembered again");
    std::fs::write(line_file(&spool, 4, &token, 1), saved).expect("put back");

    assert_eq!(kept(&spool).len(), 1);
    assert_eq!(
        drained(&spool),
        [Drained::Rejected {
            chat: 4,
            seq: Some(1),
            why: Why::Repeated,
        }]
    );
}

/// A drain that recorded a chat's lines and could not write what it drained leaves the lines:
/// they are handed on again at the next drain, the side a drain errs on, and never removed
/// with nothing kept of them.
#[test]
fn a_drain_that_cannot_write_its_keys_leaves_the_lines_and_hands_them_on_again() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    let mode = |mode| {
        std::fs::set_permissions(&spool, std::fs::Permissions::from_mode(mode)).expect("its mode")
    };

    mode(0o500);
    let mut recorded = Vec::new();
    let stopped = drain(&spool, &mut |item| {
        recorded.push(item);
        Ok(())
    });
    mode(0o700);

    assert!(stopped.is_err(), "the keys could not be written");
    assert_eq!(tools(&recorded), [(4, 1, "a".to_owned())]);
    assert_eq!(names(&spool, 4).len(), 1, "the line is still there");
    assert_eq!(tools(&drained(&spool)), [(4, 1, "a".to_owned())]);
}

/// Two drains of one spool at once, as two hosts on one project would be: every line is handed
/// on by one of them and by no other.
#[test]
fn two_drains_at_once_hand_on_each_line_once_between_them() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    for n in 0..60 {
        append(&spool, 4, &token, &call(4, &format!("{n:02}"))).expect("spooled");
    }

    let (one, other) = std::thread::scope(|scope| {
        let one = scope.spawn(|| drained(&spool));
        let other = scope.spawn(|| drained(&spool));
        (one.join().expect("a drain"), other.join().expect("a drain"))
    });

    let mut numbers: Vec<u64> = tools(&one)
        .into_iter()
        .chain(tools(&other))
        .map(|(_, seq, _)| seq)
        .collect();
    numbers.sort_unstable();
    assert_eq!(numbers, (1..=60).collect::<Vec<_>>());
    assert!(
        one.is_empty() || other.is_empty(),
        "one of them found nothing left, not even a line to reject"
    );
}

/// What is kept of the numbers a key is missing is bounded: past 64 ranges the lowest go, and a
/// line under one of those is `repeated`, where one under a range still kept is handed on.
#[test]
fn the_numbers_a_key_is_missing_are_kept_up_to_a_bound_and_the_lowest_go_first() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    std::fs::create_dir(folder_for(&spool, 4)).expect("the chat's folder");
    let put = |seq: u64| {
        std::fs::write(
            line_file(&spool, 4, &token, seq),
            a_spool_line(4, &token, seq, &seq.to_string()),
        )
        .expect("written");
    };
    // Every even number up to 140: seventy gaps of one number each.
    (1..=70).for_each(|n| put(2 * n));

    let first = drained(&spool);
    put(1);
    put(139);
    let next = drained(&spool);

    let gaps = |items: &[Drained]| {
        items
            .iter()
            .filter(|item| matches!(item, Drained::Gap { .. }))
            .count()
    };
    assert_eq!(gaps(&first), 70, "every gap is said");
    assert_eq!(
        keys_of(&spool)["keys"][0]["missing"]
            .as_array()
            .expect("missing")
            .len(),
        A_KEY_KEEPS_AT_MOST - 1,
        "64 kept, and 139 has since been handed on"
    );
    assert_eq!(tools(&next), [(4, 139, "139".to_owned())], "{next:?}");
    assert!(
        next.contains(&Drained::Rejected {
            chat: 4,
            seq: Some(1),
            why: Why::Repeated,
        }),
        "{next:?}"
    );
}

/// A chat that is closed: what it spooled is handed on under its key, and then the key is gone.
/// No key outlives its chat, and no other chat's key goes with it.
#[test]
fn a_chat_that_ends_has_its_spool_drained_and_then_its_keys_dropped() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let other = issued(&spool, 5);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    append(&spool, 5, &other, &call(5, "x")).expect("spooled");

    let mut ended = Vec::new();
    end_chat(&spool, 4, &mut |item| {
        ended.push(item);
        Ok(())
    })
    .expect("the chat ends");
    // A hook of the chat that is still about spools once more.
    append(&spool, 4, &token, &call(4, "late")).expect("spooled");
    let next = drained(&spool);

    assert_eq!(
        tools(&ended),
        [(4, 1, "a".to_owned())],
        "only its own spool"
    );
    assert_eq!(kept(&spool), [(5, SpoolKey::of(&other).id())]);
    assert_eq!(tools(&next), [(5, 1, "x".to_owned())], "{next:?}");
    assert!(
        next.contains(&Drained::Rejected {
            chat: 4,
            seq: Some(1),
            why: Why::NoKey,
        }),
        "a line spooled after its chat ended has no key: {next:?}"
    );
}

#[test]
fn a_chat_whose_lines_could_not_be_recorded_as_it_ended_keeps_its_key_for_the_next_open() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");

    let stopped = end_chat(&spool, 4, &mut |_| Err(io::Error::other("a full disk")));

    assert!(stopped.is_err());
    assert_eq!(kept(&spool).len(), 1, "its line still needs it");
    assert_eq!(tools(&drained(&spool)), [(4, 1, "a".to_owned())]);
}

/// A chat started again takes a new number and a key of its own, and the old number is closed
/// once the new one runs. What the old one spooled is recorded under the old key before that
/// key is dropped, and the new one's key and lines are untouched.
#[test]
fn a_chat_started_again_has_the_old_ones_lines_drained_before_its_key_is_dropped() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let tokens = ChatTokens::spooling_into(spool.clone());
    let old = tokens.issue_to_this_process(4).expect("a token");
    append(&spool, 4, &old, &call(4, "before-the-restart")).expect("spooled");
    // The new one starts, and its hooks may spool, before the old one is ended.
    let new = tokens.issue_to_this_process(9).expect("a token");
    append(&spool, 9, &new, &call(9, "after-the-restart")).expect("spooled");

    let mut ended = Vec::new();
    end_chat(&spool, 4, &mut |item| {
        ended.push(item);
        Ok(())
    })
    .expect("the old one ends");

    assert_eq!(tools(&ended), [(4, 1, "before-the-restart".to_owned())]);
    assert_eq!(kept(&spool), [(9, SpoolKey::of(&new).id())]);
    assert_eq!(
        tools(&drained(&spool)),
        [(9, 1, "after-the-restart".to_owned())]
    );
    assert_eq!(kept(&spool).len(), 1, "the new one is still open");
}

/// A project opened without a chat it had: the chat has ended, and its key goes once the open's
/// drain has handed on what it spooled. A chat the reopen record brings back keeps its key.
#[test]
fn a_chat_a_project_is_opened_without_has_its_key_dropped_after_the_drain() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let gone = issued(&spool, 4);
    let back = issued(&spool, 5);
    append(&spool, 4, &gone, &call(4, "a")).expect("spooled");

    let mut items = Vec::new();
    drain_at_open(&spool, &[5], &mut |item| {
        items.push(item);
        Ok(())
    })
    .expect("the spool drains");

    assert_eq!(tools(&items), [(4, 1, "a".to_owned())]);
    assert_eq!(kept(&spool), [(5, SpoolKey::of(&back).id())]);
    forget_all_but(&spool, &[]).expect("the keys are written");
    assert_eq!(kept(&spool), []);
}

#[test]
fn an_open_whose_drain_stops_drops_no_key() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let gone = issued(&spool, 4);
    append(&spool, 4, &gone, &call(4, "a")).expect("spooled");

    let stopped = drain_at_open(&spool, &[], &mut |_| Err(io::Error::other("a full disk")));

    assert!(stopped.is_err());
    assert_eq!(kept(&spool).len(), 1, "its line still needs it");
    assert_eq!(tools(&drained(&spool)), [(4, 1, "a".to_owned())]);
}

#[test]
fn a_host_with_nowhere_to_record_drops_an_ended_chats_keys_and_drains_nothing() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    issued(&spool, 5);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");

    forget_chat(&spool, 4).expect("the keys are written");

    assert_eq!(kept(&spool).len(), 1);
    assert_eq!(kept(&spool)[0].0, 5);
    assert_eq!(names(&spool, 4).len(), 1, "nothing was drained");
}

/// #983: a host with no event log drains nothing as a project opens, and a chat the reopen
/// record does not bring back has ended all the same. Its keys go, so they do not pile up launch
/// after launch, unless its spool still holds a line: that one keeps its keys for a host that
/// can record the line. A chat brought back keeps its keys.
#[test]
fn an_open_with_nowhere_to_record_drops_the_keys_of_ended_chats_that_spooled_nothing() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let idle = issued(&spool, 4);
    let waiting = issued(&spool, 5);
    let back = issued(&spool, 6);
    append(&spool, 5, &waiting, &call(5, "a")).expect("spooled");

    forget_ended_at_open(&spool, &[6]).expect("the keys are written");

    let ids: Vec<String> = kept(&spool).into_iter().map(|(_, id)| id).collect();
    assert!(!ids.contains(&SpoolKey::of(&idle).id()), "{ids:?}");
    assert!(
        ids.contains(&SpoolKey::of(&waiting).id()),
        "its line still needs it"
    );
    assert!(ids.contains(&SpoolKey::of(&back).id()), "brought back");
    assert_eq!(names(&spool, 5).len(), 1, "nothing was drained");
    assert_eq!(
        tools(&drained(&spool)),
        [(5, 1, "a".to_owned())],
        "a host that can record it hands the line on"
    );
}

/// #983: a chat's folder, and the file a build before #983 kept for it, are removed once the
/// chat has ended and they hold nothing: at its end, at an open without it, and at an end with
/// nowhere to record. A chat still open keeps its folder, empty or not, and a folder that still
/// holds a name is never removed.
#[test]
fn an_ended_chats_empty_spool_is_removed_and_an_open_ones_is_kept() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let ending = issued(&spool, 4);
    let open = issued(&spool, 5);
    let gone = issued(&spool, 6);
    let quiet = issued(&spool, 7);
    for (chat, token) in [(4, &ending), (5, &open), (6, &gone), (7, &quiet)] {
        append(&spool, chat, token, &call(chat, "a")).expect("spooled");
        std::fs::write(file_for(&spool, chat), b"").expect("an emptied older file");
    }
    let nothing = |_: Drained| -> io::Result<()> { Ok(()) };

    // An open chat whose key is not recorded yet (#1426) holds no key, and is not another
    // chat's end to tidy.
    std::fs::create_dir(folder_for(&spool, 9)).expect("an unkeyed chat's folder");

    end_chat(&spool, 4, &mut { nothing }).expect("the chat ends");
    assert!(
        !folder_for(&spool, 4).exists(),
        "its folder went at its end"
    );
    assert!(
        !file_for(&spool, 4).exists(),
        "its emptied file went at its end"
    );
    assert!(
        folder_for(&spool, 9).is_dir(),
        "only the chat that ended is tidied"
    );

    drain_running(&spool, 5, &mut { nothing }).expect("drained");
    assert!(
        folder_for(&spool, 5).is_dir(),
        "an open chat keeps its folder"
    );
    assert!(file_for(&spool, 5).is_file(), "and its file");
    assert!(
        folder_for(&spool, 9).is_dir(),
        "a running drain tidies nothing"
    );

    drain_at_open(&spool, &[5, 7], &mut { nothing }).expect("the spool drains");
    assert!(
        !folder_for(&spool, 6).exists(),
        "an open without it removes its folder"
    );
    assert!(!file_for(&spool, 6).exists());
    assert!(folder_for(&spool, 5).is_dir(), "brought back");
    assert!(folder_for(&spool, 7).is_dir(), "brought back");
    assert!(!folder_for(&spool, 9).exists(), "not brought back: ended");

    // A folder that still holds a name stays, and so does a file that holds a byte.
    std::fs::write(folder_for(&spool, 7).join("stray"), b"x").expect("a stray name");
    std::fs::write(file_for(&spool, 7), b"x\n").expect("a byte");
    forget_chat(&spool, 7).expect("the keys are written");
    assert!(folder_for(&spool, 7).is_dir(), "not empty");
    assert!(file_for(&spool, 7).is_file(), "not empty");
    std::fs::remove_file(folder_for(&spool, 7).join("stray")).expect("removed");
    std::fs::write(file_for(&spool, 7), b"").expect("emptied");
    forget_chat(&spool, 7).expect("the keys are written");
    assert!(!folder_for(&spool, 7).exists(), "ended, empty, removed");
    assert!(!file_for(&spool, 7).exists(), "ended, empty, removed");
}

/// What a tidy of an ended chat's spool never touches: a link under a chat's name, whatever
/// it points at, and a name that is not a chat's.
#[cfg(unix)]
#[test]
fn a_tidy_follows_no_link_and_touches_no_other_name() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    issued(&spool, 1);
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    let empty_dir = elsewhere.path().join("empty");
    std::fs::create_dir(&empty_dir).expect("an empty folder");
    let empty_file = elsewhere.path().join("empty.jsonl");
    std::fs::write(&empty_file, b"").expect("an empty file");
    std::os::unix::fs::symlink(&empty_dir, folder_for(&spool, 8)).expect("a link");
    std::os::unix::fs::symlink(&empty_file, file_for(&spool, 8)).expect("a link");
    std::fs::create_dir(spool.join("08")).expect("not a chat's name");
    std::fs::create_dir(spool.join("other")).expect("not a chat's name");

    forget_all_but(&spool, &[]).expect("the keys are written");

    assert!(
        folder_for(&spool, 8).symlink_metadata().is_ok(),
        "a link is left"
    );
    assert!(
        file_for(&spool, 8).symlink_metadata().is_ok(),
        "a link is left"
    );
    assert!(
        empty_dir.is_dir() && empty_file.is_file(),
        "nothing it points at goes"
    );
    assert!(spool.join("08").is_dir() && spool.join("other").is_dir());
}

/// A line a drain took from the chat's folder is not taken again from the file a build before
/// wrote. This build's hooks never write that file, so under a key this build issued nothing
/// in it is a line: a copy put there after the drain is `repeated`, at the next drain and as
/// the chat ends.
#[test]
fn a_drained_line_put_back_as_an_older_builds_file_is_rejected_at_a_drain_and_at_the_chats_end() {
    type Ends = fn(&Path, &mut dyn FnMut(Drained) -> io::Result<()>) -> io::Result<()>;
    let at_a_drain: Ends = |spool, each| drain(spool, each);
    let as_the_chat_ends: Ends = |spool, each| end_chat(spool, 4, each);
    for again in [at_a_drain, as_the_chat_ends] {
        let dir = tempfile::tempdir().expect("a directory");
        let spool = dir.path().join(".charter/app").join(DIR);
        let token = issued(&spool, 4);
        append(&spool, 4, &token, &call(4, "a")).expect("spooled");
        let saved = std::fs::read(line_file(&spool, 4, &token, 1)).expect("the line");
        assert_eq!(tools(&drained(&spool)), [(4, 1, "a".to_owned())]);

        // Something that can write the spool puts the line's bytes back as the old file.
        std::fs::write(file_for(&spool, 4), saved).expect("put back");
        let mut items = Vec::new();
        again(&spool, &mut |item| {
            items.push(item);
            Ok(())
        })
        .expect("it drains");

        assert_eq!(
            items,
            [Drained::Rejected {
                chat: 4,
                seq: Some(1),
                why: Why::Repeated,
            }]
        );
    }
}

/// The hook asks what was drained once its number is taken. A drain that ran after the hook
/// listed the folder has left the folder empty and the low numbers free: the hook lets the one
/// it took go, and takes the one after the last drained.
#[test]
fn a_hook_that_took_a_number_a_drain_had_already_handed_on_lets_it_go_and_takes_one_above() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    private(&spool).expect("the spool");
    let folder = Folder::of_a_hook(&spool, 4).expect("the chat's folder");
    let key = "0123456789abcdef";
    let asked = std::cell::Cell::new(0);
    // Seven lines were drained under the key, and their files are gone.
    let drained = || {
        asked.set(asked.get() + 1);
        7
    };

    let (seq, _part) = folder
        .take_a_number(key, 100, &mut 0, &drained)
        .expect("a number");

    assert_eq!(seq, 8);
    assert_eq!(
        names(&spool, 4),
        [name_of(key, 8, Kind::Part)],
        "the number it let go holds nothing"
    );
    assert_eq!(asked.get(), 2, "once per number it took, and never before");
}

/// The lock on the spool directory, as another process holding it would: a second open is a
/// second holder to flock.
fn holding_the_keys(spool: &Path) -> File {
    let holder = File::open(spool).expect("the spool opens");
    holder.lock().expect("the holder takes the lock");
    holder
}

/// #1426: issuing a token waits for the spool's keys only so long. The token's key is then not
/// recorded, and a line spooled under it is `no-key` at the drain: never handed on unchecked.
#[test]
fn a_key_the_spools_lock_kept_out_is_not_recorded_and_its_lines_are_never_handed_on() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let _first = issued(&spool, 3);
    let token = ChatToken("a token whose key the lock kept out".to_owned());
    let holder = holding_the_keys(&spool);

    let started = std::time::Instant::now();
    let refused = remember(&spool, 4, &token).expect_err("the holder keeps the keys");
    let waited = started.elapsed();

    assert_eq!(refused.kind(), io::ErrorKind::TimedOut, "{refused}");
    assert!(
        refused.to_string().contains("the hook spool's keys"),
        "{refused}"
    );
    assert!(
        waited >= THE_KEYS_ARE_WAITED_FOR_AT_MOST
            && waited < THE_KEYS_ARE_WAITED_FOR_AT_MOST + Duration::from_secs(3),
        "waited {waited:?}"
    );
    drop(holder);
    append(&spool, 4, &token, &call(4, "unchecked")).expect("the hook spools it");

    let items = drained(&spool);

    assert_eq!(tools(&items), [], "{items:?}");
    assert!(
        items.contains(&Drained::Rejected {
            chat: 4,
            seq: Some(1),
            why: Why::NoKey
        }),
        "{items:?}"
    );
}

/// #1426: a key the spool's lock kept out as its token was issued is recorded at the next
/// chance (the next token issued, here), and what its chat spooled is then that chat's at the
/// drain. Recording it again changes nothing: a line drained under it, put back, is repeated.
#[test]
fn a_key_the_spools_lock_kept_out_is_recorded_later_and_its_lines_drain_once() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let tokens = ChatTokens::spooling_into(spool.clone());
    let first = tokens.issue_to_this_process(3).expect("a token");
    let holder = holding_the_keys(&spool);
    let token = tokens
        .issue_to_this_process(4)
        .expect("issued all the same");
    drop(holder);
    append(&spool, 4, &token, &call(4, "kept-out")).expect("the hook spools it");
    assert_eq!(kept(&spool), [(3, SpoolKey::of(&first).id())], "not yet");

    let other = tokens.issue_to_this_process(5).expect("a token");

    assert_eq!(
        kept(&spool),
        [
            (3, SpoolKey::of(&first).id()),
            (5, SpoolKey::of(&other).id()),
            (4, SpoolKey::of(&token).id()),
        ]
    );
    let line = std::fs::read(line_file(&spool, 4, &token, 1)).expect("the line");
    assert_eq!(tools(&drained(&spool)), [(4, 1, "kept-out".to_owned())]);
    // Nothing left to record, and nothing recorded twice.
    tokens.record_unrecorded();
    let _again = tokens.issue_to_this_process(6).expect("a token");
    std::fs::write(line_file(&spool, 4, &token, 1), line).expect("put back");
    let items = drained(&spool);
    assert_eq!(tools(&items), [], "{items:?}");
    assert!(
        items.contains(&Drained::Rejected {
            chat: 4,
            seq: Some(1),
            why: Why::Repeated
        }),
        "{items:?}"
    );
}

/// #1426: a line heard from a chat is the next chance too, and it never waits on the lock: a
/// holder still on it leaves the key for the chance after.
#[test]
fn a_key_the_spools_lock_kept_out_is_recorded_once_a_line_finds_the_lock_free() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let tokens = ChatTokens::spooling_into(spool.clone());
    let first = tokens.issue_to_this_process(3).expect("a token");
    let holder = holding_the_keys(&spool);
    let token = tokens
        .issue_to_this_process(4)
        .expect("issued all the same");

    let started = std::time::Instant::now();
    tokens.record_unrecorded();
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "waited {:?}",
        started.elapsed()
    );
    drop(holder);
    assert_eq!(kept(&spool), [(3, SpoolKey::of(&first).id())]);

    tokens.record_unrecorded();

    assert_eq!(
        kept(&spool),
        [
            (3, SpoolKey::of(&first).id()),
            (4, SpoolKey::of(&token).id())
        ]
    );
}

/// #1426: only a chat's current token is ever recorded later. One its chat was issued again
/// since is never recorded (the drain keeps a chat's newest key, and its lines are `no-key`),
/// and nor is one whose chat has ended: no key is put back after its chat let it go.
#[test]
fn a_key_kept_out_is_recorded_later_only_while_its_token_is_the_chats() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let tokens = ChatTokens::spooling_into(spool.clone());
    let first = tokens.issue_to_this_process(3).expect("a token");
    let holder = holding_the_keys(&spool);
    let old = tokens.issue_to_this_process(4).expect("a token");
    let new = tokens.issue_to_this_process(4).expect("a token again");
    let forgotten = tokens.issue_to_this_process(5).expect("a token");
    tokens.forget(5);
    let ended = tokens.issue_to_this_process(6).expect("a token");
    tokens.program_ended(6, std::process::id());
    drop(holder);

    tokens.record_unrecorded();

    let ids: Vec<String> = kept(&spool).into_iter().map(|(_, id)| id).collect();
    assert_eq!(
        ids,
        [SpoolKey::of(&first).id(), SpoolKey::of(&new).id()],
        "neither the replaced token nor an ended chat's is recorded"
    );
    for (chat, token) in [(4, &old), (5, &forgotten), (6, &ended)] {
        assert!(!ids.contains(&SpoolKey::of(token).id()), "chat {chat}");
    }
    append(&spool, 4, &old, &call(4, "old")).expect("spooled");
    let items = drained(&spool);
    assert_eq!(tools(&items), [], "{items:?}");
    assert!(
        items.contains(&Drained::Rejected {
            chat: 4,
            seq: Some(1),
            why: Why::NoKey
        }),
        "{items:?}"
    );
}

/// #1426: a drain, and a chat's end, wait for the spool's keys only so long, and then read no
/// line and drop no key: the next drain hands on what is there.
#[test]
fn a_drain_the_spools_lock_kept_out_reads_nothing_and_the_next_one_drains_it_all() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    let holder = holding_the_keys(&spool);

    let mut seen = Vec::new();
    let refused = drain(&spool, &mut |item| {
        seen.push(item);
        Ok(())
    })
    .expect_err("the holder keeps the keys");
    assert_eq!(refused.kind(), io::ErrorKind::TimedOut, "{refused}");
    let ended = end_chat(&spool, 4, &mut |item| {
        seen.push(item);
        Ok(())
    })
    .expect_err("the holder keeps the keys");
    assert_eq!(ended.kind(), io::ErrorKind::TimedOut, "{ended}");
    assert_eq!(
        forget_all_but(&spool, &[]).map_err(|why| why.kind()),
        Err(io::ErrorKind::TimedOut)
    );
    assert_eq!(seen, [], "nothing was read");
    drop(holder);

    assert_eq!(tools(&drained(&spool)), [(4, 1, "a".to_owned())]);
}

/// #1419: the drain waits for one chat's spool only so long. That chat is left as it is, every
/// key it has kept, and the other chats are drained; once it is let go of, the next drain hands
/// on its lines, the ones under the key it was issued first too.
#[test]
fn a_chats_spool_another_process_holds_is_left_with_its_keys_and_the_rest_are_drained() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let first = issued(&spool, 4);
    append(&spool, 4, &first, &call(4, "first-start")).expect("spooled");
    let again = issued(&spool, 4);
    append(&spool, 4, &again, &call(4, "second-start")).expect("spooled");
    let other = issued(&spool, 5);
    append(&spool, 5, &other, &call(5, "other")).expect("spooled");
    let holder = File::open(folder_for(&spool, 4)).expect("the folder opens");
    holder.lock().expect("the holder takes the lock");

    let mut items = Vec::new();
    let started = std::time::Instant::now();
    drain_at_open(&spool, &[], &mut |item| {
        items.push(item);
        Ok(())
    })
    .expect("the drain goes on past the held chat");
    let waited = started.elapsed();

    assert_eq!(tools(&items), [(5, 1, "other".to_owned())], "{items:?}");
    assert!(
        waited >= THE_DRAIN_WAITS_FOR_A_SPOOL_AT_MOST
            && waited < THE_DRAIN_WAITS_FOR_A_SPOOL_AT_MOST + Duration::from_secs(3),
        "waited {waited:?}"
    );
    assert_eq!(names(&spool, 4).len(), 2, "chat 4's lines are left");
    assert_eq!(
        keys_of(&spool)["keys"]
            .as_array()
            .expect("keys")
            .iter()
            .filter(|held| held["chat"] == 4)
            .count(),
        2,
        "chat 4 keeps both its keys"
    );
    drop(holder);

    assert_eq!(
        tools(&drained(&spool)),
        [
            (4, 1, "first-start".to_owned()),
            (4, 1, "second-start".to_owned())
        ]
    );
}

/// #1419: a chat's end waits for its spool only so long, and then keeps its keys, as an end
/// whose lines could not be recorded does.
#[test]
fn a_chat_whose_spool_is_held_as_it_ends_keeps_its_keys_for_the_next_drain() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    let holder = File::open(folder_for(&spool, 4)).expect("the folder opens");
    holder.lock().expect("the holder takes the lock");

    let refused = end_chat(&spool, 4, &mut |_| Ok(())).expect_err("its spool is held");

    assert_eq!(refused.kind(), io::ErrorKind::TimedOut, "{refused}");
    drop(holder);
    assert_eq!(tools(&drained(&spool)), [(4, 1, "a".to_owned())]);
}

/// #1419: the file a build before #983 wrote is drained under its lock, which that build's
/// hooks hold across their sync: waited for only so long, and the chat left as it is.
#[test]
fn an_older_builds_spool_file_another_process_holds_is_left_unread() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    as_a_build_before_wrote_its_keys(&spool);
    let old = file_for(&spool, 4);
    std::fs::write(&old, a_spool_line(4, &token, 1, "old")).expect("the old spool");
    let holder = File::open(&old).expect("the file opens");
    holder.lock().expect("the holder takes the lock");

    assert_eq!(drained(&spool), []);
    assert_eq!(
        std::fs::read_to_string(&old).expect("still there"),
        a_spool_line(4, &token, 1, "old"),
        "left as it was"
    );
    drop(holder);
    assert_eq!(tools(&drained(&spool)), [(4, 1, "old".to_owned())]);
}

/// #1419: the drain reads no more of an older build's file than that build could have written.
/// One past it is `unreadable`, left as it is, and none of its lines is handed on.
#[test]
fn an_older_builds_spool_file_past_what_it_could_hold_is_unreadable_and_left() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    as_a_build_before_wrote_its_keys(&spool);
    let old = file_for(&spool, 4);
    let line = a_spool_line(4, &token, 1, "old");
    std::fs::write(&old, &line).expect("the old spool");
    let past = AN_OLDER_BUILDS_FILE_IS_READ_UP_TO + 1;
    File::options()
        .write(true)
        .open(&old)
        .and_then(|file| file.set_len(past))
        .expect("the file grows past the cap");

    let items = drained(&spool);

    assert_eq!(
        items,
        [Drained::Rejected {
            chat: 4,
            seq: None,
            why: Why::Unreadable
        }]
    );
    assert_eq!(
        std::fs::metadata(&old).expect("still there").len(),
        past,
        "left as it was"
    );
}

/// The spool directories a test's hooks made durable, by the seam below.
static SYNCED: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

/// #1419: a spool directory whose sync a hook did not finish is synced by the next line, though
/// that hook made no folder.
#[test]
fn a_spool_whose_sync_a_hook_did_not_finish_is_synced_by_the_next_line() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let a_sync_that_fails = Bounds {
        the_spool_reaches_the_disk: |_| Err(io::Error::other("the disk did not answer")),
        ..Bounds::A_HOOKS
    };
    append_within(&spool, 4, &token, &call(4, "a"), a_sync_that_fails)
        .expect_err("the hook that made the folder did not sync it");
    let recorded = Bounds {
        the_spool_reaches_the_disk: |dir| {
            SYNCED
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(dir.to_path_buf());
            sync_dir(dir)
        },
        ..Bounds::A_HOOKS
    };

    append_within(&spool, 4, &token, &call(4, "b"), recorded).expect("spooled");

    assert!(
        SYNCED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&spool),
        "the next line synced the spool directory"
    );
}

/// Chat `chat`'s spool drained while the host runs ([`drain_running`]), every item kept.
fn drained_running(spool: &Path, chat: u32) -> Vec<Drained> {
    let mut all = Vec::new();
    drain_running(spool, chat, &mut |item| {
        all.push(item);
        Ok(())
    })
    .expect("the chat's spool drains");
    all
}

/// #952: a chat's spool is drained while the host runs, not only at the next open, and the
/// chat goes on: its key is kept, so a line its hooks spool after that drain is handed on by
/// the next one, numbered on, with no gap and nothing `repeated`.
#[test]
fn a_running_chats_spool_is_drained_without_a_restart_and_its_key_goes_on() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let other = issued(&spool, 5);
    append(&spool, 4, &token, &call(4, "a")).expect("spooled");
    append(&spool, 5, &other, &call(5, "x")).expect("spooled");

    assert!(holds_lines(&spool, 4));
    let first = drained_running(&spool, 4);
    assert!(!holds_lines(&spool, 4), "its lines are drained");
    assert!(holds_lines(&spool, 5), "another chat's are not");
    append(&spool, 4, &token, &call(4, "b")).expect("spooled");
    let second = drained_running(&spool, 4);

    assert_eq!(
        tools(&first),
        [(4, 1, "a".to_owned())],
        "only its own spool"
    );
    assert_eq!(tools(&second), [(4, 2, "b".to_owned())], "{second:?}");
    assert!(
        second
            .iter()
            .all(|item| !matches!(item, Drained::Gap { .. } | Drained::Rejected { .. })),
        "{second:?}"
    );
    assert_eq!(
        kept(&spool),
        [
            (4, SpoolKey::of(&token).id()),
            (5, SpoolKey::of(&other).id())
        ],
        "a running chat keeps its key, and another chat's is untouched"
    );
    assert_eq!(
        tools(&drained(&spool)),
        [(5, 1, "x".to_owned())],
        "another chat's line waits for its own drain"
    );
}

/// #952: a line the host took late, after its hook had stopped waiting and spooled it too, is
/// recorded once. Its spooled copy is drained without being handed on, and the chat's numbers
/// go on past it with no gap.
#[test]
fn a_line_the_host_recorded_live_and_its_hook_also_spooled_is_handed_on_once() {
    use crate::hookwire::{Delivery, content_of, deliveries, with_delivery};
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 4);
    let late = Delivery::new().expect("an id");
    let other = Delivery::new().expect("an id");
    // The host took it live, and recorded it, after the hook had given up on it.
    let content = content_of(&call(4, "late"));
    assert!(deliveries().begin(4, &late, &content, NOW));
    deliveries().finish(4, &late, &content, true);
    // The hook spooled it; then another line, which the host never took.
    let spooled = |id, delivery| with_delivery(&call(4, id), Some(delivery)).expect("a line");
    append(&spool, 4, &token, &spooled("late", &late)).expect("spooled");
    append(&spool, 4, &token, &spooled("missed", &other)).expect("spooled");

    let items = drained_running(&spool, 4);

    assert_eq!(tools(&items), [(4, 2, "missed".to_owned())], "{items:?}");
    assert!(
        items.contains(&Drained::Spool {
            chat: 4,
            from: 1,
            to: 2
        }),
        "the sequence is drained past the line recorded live: {items:?}"
    );
    assert!(
        items
            .iter()
            .all(|item| !matches!(item, Drained::Gap { .. } | Drained::Rejected { .. })),
        "{items:?}"
    );
    assert!(!holds_lines(&spool, 4), "both files are gone");
}

/// The same line recorded live under one chat is not taken for another chat's: what the host
/// remembers is per chat, so a chat cannot have another's spooled line skipped.
#[test]
fn a_delivery_recorded_live_for_one_chat_does_not_skip_another_chats_line() {
    use crate::hookwire::{Delivery, content_of, deliveries, with_delivery};
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 5);
    let same = Delivery::new().expect("an id");
    let content = content_of(&call(5, "mine"));
    assert!(deliveries().begin(6, &same, &content, NOW));
    deliveries().finish(6, &same, &content, true);
    let line = with_delivery(&call(5, "mine"), Some(&same)).expect("a line");
    append(&spool, 5, &token, &line).expect("spooled");

    assert_eq!(
        tools(&drained_running(&spool, 5)),
        [(5, 1, "mine".to_owned())]
    );
}

/// A line the host heard live and could not record is not one it recorded: its spooled copy
/// is handed on.
#[test]
fn a_line_the_host_heard_live_and_could_not_record_is_handed_on_from_the_spool() {
    use crate::hookwire::{Delivery, content_of, deliveries, with_delivery};
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 7);
    let failed = Delivery::new().expect("an id");
    let content = content_of(&call(7, "unrecorded"));
    assert!(deliveries().begin(7, &failed, &content, NOW));
    deliveries().finish(7, &failed, &content, false);
    let line = with_delivery(&call(7, "unrecorded"), Some(&failed)).expect("a line");
    append(&spool, 7, &token, &line).expect("spooled");

    assert_eq!(
        tools(&drained_running(&spool, 7)),
        [(7, 1, "unrecorded".to_owned())]
    );
}

/// No wait: a test asks [`crate::hookwire::Deliveries::begin`] what is so at once.
const NOW: std::time::Duration = std::time::Duration::ZERO;

/// #952, the other order: a line whose spooled copy the drain recorded first is not recorded
/// again when the host hears it late, live.
#[test]
fn a_line_the_drain_recorded_first_is_not_recorded_again_when_heard_live() {
    use crate::hookwire::{Delivery, content_of, deliveries, with_delivery};
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 8);
    let late = Delivery::new().expect("an id");
    let line = with_delivery(&call(8, "late"), Some(&late)).expect("a line");
    append(&spool, 8, &token, &line).expect("spooled");

    assert_eq!(
        tools(&drained_running(&spool, 8)),
        [(8, 1, "late".to_owned())],
        "the drain went first and recorded it"
    );

    // The host hears the same line late: it is not recorded again.
    assert!(!deliveries().begin(8, &late, &content_of(&call(8, "late")), NOW));
}

/// A line that carries another line's id, and says something else, is never taken for it,
/// in either order: a chat's hook chooses the id, so an id alone keeps no line from being
/// recorded.
#[test]
fn a_line_under_another_lines_id_is_recorded_as_its_own() {
    use crate::hookwire::{Delivery, content_of, deliveries, with_delivery};
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 9);
    let id = Delivery::new().expect("an id");
    // Something sent first under the id, and recorded.
    let first = content_of(&call(9, "first"));
    assert!(deliveries().begin(9, &id, &first, NOW));
    deliveries().finish(9, &id, &first, true);
    // The real line under the same id, spooled: handed on.
    let line = with_delivery(&call(9, "real"), Some(&id)).expect("a line");
    append(&spool, 9, &token, &line).expect("spooled");
    assert_eq!(
        tools(&drained_running(&spool, 9)),
        [(9, 1, "real".to_owned())]
    );
    // And heard live under it: recorded too.
    assert!(deliveries().begin(9, &id, &content_of(&call(9, "live")), NOW));
}

/// Every file under `dir`, as bytes, for a look at what reached the disk.
fn every_file_under(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut all = Vec::new();
    let mut left = vec![dir.to_path_buf()];
    while let Some(at) = left.pop() {
        for entry in std::fs::read_dir(&at).expect("a directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                left.push(path);
            } else {
                all.push((path.clone(), std::fs::read(&path).expect("a file")));
            }
        }
    }
    all
}

/// #977: a tool line's argument digest is the unkeyed SHA-256 the hook sends, which anyone can
/// match against a guess. It is never written to disk: the spooled line leaves it out, and the
/// drained line has none.
#[test]
fn no_unkeyed_argument_digest_is_written_to_disk() {
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 11);
    let digest = crate::eventlog::args_hash(&serde_json::json!({"command": "ls -la"}));
    let mut with_args = call(11, "digested");
    with_args.args = Some(digest.clone());
    let delivery = crate::hookwire::Delivery::new().expect("an id");
    let line = crate::hookwire::with_delivery(&with_args, Some(&delivery)).expect("a line");

    append(&spool, 11, &token, &with_args).expect("spooled");
    append(&spool, 11, &token, &line).expect("spooled");

    for (path, bytes) in every_file_under(dir.path()) {
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !text.contains(&digest),
            "{} holds the unkeyed digest: {text}",
            path.display()
        );
    }
    let items = drained(&spool);
    let calls: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            Drained::Line {
                line: Spooled::Tool(call),
                ..
            } => Some(call.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(calls.len(), 2, "{items:?}");
    assert!(calls.iter().all(|call| call.args.is_none()), "{calls:?}");
}

/// #977 with #952: the host heard a tool line live with its digest, and its spooled copy has
/// none; the copy is still known for the same line, and is recorded once.
#[test]
fn a_tool_line_heard_live_with_its_digest_is_known_in_its_spooled_copy_without_one() {
    use crate::hookwire::{Delivery, content_of, deliveries, with_delivery};
    let dir = tempfile::tempdir().expect("a directory");
    let spool = dir.path().join(".charter/app").join(DIR);
    let token = issued(&spool, 12);
    let late = Delivery::new().expect("an id");
    let mut with_args = call(12, "late");
    with_args.args = Some(crate::eventlog::args_hash(&serde_json::json!({"x": 1})));
    // What the host keeps of the line it heard live, digest and all.
    let content = content_of(&with_args.at_rest());
    assert!(deliveries().begin(12, &late, &content, NOW));
    deliveries().finish(12, &late, &content, true);
    let line = with_delivery(&with_args, Some(&late)).expect("a line");
    append(&spool, 12, &token, &line).expect("spooled");

    assert_eq!(tools(&drained_running(&spool, 12)), Vec::new());
}
