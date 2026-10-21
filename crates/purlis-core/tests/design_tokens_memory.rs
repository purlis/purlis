// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A design-tokens import is bounded in memory** (#2926): for every shape of file that meets one
//! of its bounds, the most the heap holds while it is read, counted by a counting allocator in a
//! test binary of its own, so nothing else allocates beside it.

use purlis_core::theme::design_tokens::{
    MOST_BYTES, MOST_NAME_BYTES, MOST_PATH_BYTES, MOST_VALUES, VENDOR,
};
use purlis_core::theme::{self, SourceKind};
use serde_json::{Value, json};

#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

const MIB: u64 = 1024 * 1024;

/// What importing `bytes` answers, and the most the heap held above what it held before.
fn peak(bytes: &[u8]) -> (Result<theme::Imported, String>, u64) {
    let profiler = dhat::Profiler::builder().testing().build();
    let before = dhat::HeapStats::get().curr_bytes as u64;
    let answer = theme::import(SourceKind::DesignTokens, "x.tokens.json", bytes);
    let most = dhat::HeapStats::get().max_bytes as u64;
    drop(profiler);
    (answer, most.saturating_sub(before))
}

/// A file of about `len` bytes: purlis's mark, and `filler` repeated in a list in its theme.
fn listed(filler: &str, len: u64) -> Vec<u8> {
    let mut text = format!("{{\"$extensions\":{{\"{VENDOR}\":{{\"version\":1,\"theme\":{{\"x\":[");
    while ((text.len() + filler.len() + 8) as u64) < len {
        text.push_str(filler);
    }
    text.push_str("0]}}}}");
    text.into_bytes()
}

/// `name` nested `depth` deep, holding `count` number tokens.
fn nested(name: &str, depth: usize, count: usize) -> Vec<u8> {
    let tokens: serde_json::Map<String, Value> = (0..count)
        .map(|at| (format!("t{at}"), json!({ "$type": "number", "$value": 1 })))
        .collect();
    let mut group = Value::Object(tokens);
    for _ in 0..depth {
        group = json!({ name: group });
    }
    group.to_string().into_bytes()
}

/// Each shape that meets a bound, what its import says, and the most its read may hold.
#[test]
fn every_shape_that_meets_a_bound_is_read_within_its_memory() {
    let cases: Vec<(&str, Vec<u8>, Option<&str>, u64)> = vec![
        // The value count, at the byte cap: refused before it is parsed.
        (
            "past the value count",
            listed("0,", MOST_BYTES - 16),
            Some("values"),
            MIB,
        ),
        // Long strings at the byte cap, within the count: the theme.json it makes is too large.
        (
            "long strings at the cap",
            listed(&format!("\"{}\",", "a".repeat(62)), MOST_BYTES - 16),
            Some("MiB"),
            80 * MIB,
        ),
        // Many small values, just within the count.
        (
            "many small values",
            listed("0,", u64::try_from(MOST_VALUES).unwrap() * 2 - 64),
            Some("MiB"),
            56 * MIB,
        ),
        // More appearances than a theme may have.
        (
            "too many appearances",
            json!({ "appearance": (0..65)
                .map(|at| (format!("a{at}"), json!({ "x": { "$type": "number", "$value": 1 } })))
                .collect::<serde_json::Map<String, Value>>() })
            .to_string()
            .into_bytes(),
            Some("appearances"),
            MIB,
        ),
        // A carried file past a theme folder's file cap: refused before it is decoded.
        (
            "a carried file too large",
            json!({ "$extensions": { VENDOR: { "version": 1,
                "files": { "fonts/a.woff2": "A".repeat(3 * 1024 * 1024) } } } })
            .to_string()
            .into_bytes(),
            Some("larger than"),
            8 * MIB,
        ),
        // Names longer than a name may be, nested, over many tokens.
        (
            "long names",
            nested(&"n".repeat(64 * 1024), 20, 200),
            Some("bytes long"),
            4 * MIB,
        ),
        // Names as long as a name may be, as deep as a path may go, over many tokens.
        (
            "long paths over many tokens",
            nested(
                &"n".repeat(MOST_NAME_BYTES),
                MOST_PATH_BYTES / (MOST_NAME_BYTES + 1),
                40_000,
            ),
            None,
            72 * MIB,
        ),
    ];
    let mut over = Vec::new();
    for (shape, bytes, refusal, most) in cases {
        assert!(
            bytes.len() as u64 <= MOST_BYTES,
            "{shape} is within the byte cap"
        );
        let (answer, held) = peak(&bytes);
        match (refusal, &answer) {
            (Some(said), Err(why)) => assert!(why.contains(said), "{shape}: {why}"),
            (None, Ok(_)) => {}
            (_, answer) => panic!("{shape}: {:?}", answer.as_ref().map(|it| &it.folder)),
        }
        if held > most {
            over.push(format!(
                "{shape}: {} MiB, more than {} MiB",
                held / MIB,
                most / MIB
            ));
        }
    }
    let purlis = theme::design_tokens::pack("purlis-copy", &theme::purlis_files()).unwrap();
    let (answer, held) = peak(&purlis);
    answer.unwrap();
    if held > 8 * MIB {
        over.push(format!("purlis: {} MiB", held / MIB));
    }
    assert!(over.is_empty(), "{over:#?}");
}
