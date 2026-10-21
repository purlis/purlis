// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A design-tokens import is bounded in memory** (#2926): for every shape of file that meets one
//! of its bounds, the most the heap holds while it is read, counted by a counting allocator in a
//! test binary of its own, so nothing else allocates beside it.

use purlis_core::theme::design_tokens::{
    MOST_BYTES, MOST_NAME_BYTES, MOST_PATH_BYTES, MOST_SCALAR_BYTES, MOST_VALUES, VENDOR,
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

/// An appearance group `dark` holding `base`, a token of type `kind` whose value is the JSON text
/// `value`, and `count` tokens of that type that refer to it.
fn referred(kind: &str, value: &str, count: usize) -> Vec<u8> {
    let mut text = format!(
        "{{\"appearance\":{{\"dark\":{{\"base\":{{\"$type\":\"{kind}\",\"$value\":{value}}}"
    );
    for at in 0..count {
        text.push_str(&format!(
            ",\"r{at}\":{{\"$type\":\"{kind}\",\"$value\":\"{{appearance.dark.base}}\"}}"
        ));
    }
    text.push_str("}}}");
    text.into_bytes()
}

/// An appearance group `dark` whose `$type` is `kind`, holding `count` tokens that take it.
fn inheriting(kind: &str, count: usize) -> Vec<u8> {
    let mut text = format!("{{\"appearance\":{{\"dark\":{{\"$type\":\"{kind}\"");
    for at in 0..count {
        text.push_str(&format!(",\"t{at}\":{{\"$value\":1}}"));
    }
    text.push_str("}}}");
    text.into_bytes()
}

/// The shapes where one value is reached by many tokens: by a reference, or by a group's type.
/// Each holds what 40,000 small tokens take to read; one copy of the long value for each would
/// be gigabytes.
fn shared_values() -> Vec<(&'static str, Vec<u8>, Option<&'static str>, u64)> {
    let long = "9".repeat(60 * 1024);
    let word = format!("\"{}\"", "w".repeat(60 * 1024));
    let list = format!("[{}0]", "0,".repeat(100_000));
    vec![
        // A number longer than one may be: refused before it is parsed.
        (
            "a number too long",
            referred("number", &"9".repeat(MOST_SCALAR_BYTES + 1), 0),
            Some("number longer"),
            MIB,
        ),
        // A long number, referred to by many tokens: read where it is, never copied.
        (
            "a long number referred to",
            referred("number", &long, 40_000),
            None,
            48 * MIB,
        ),
        // A long unit, referred to by many tokens: never in a message.
        (
            "a long unit referred to",
            referred(
                "dimension",
                &format!("{{\"value\":1,\"unit\":{word}}}"),
                40_000,
            ),
            None,
            48 * MIB,
        ),
        // A long list as an easing, referred to by many tokens: never copied.
        (
            "a long list referred to",
            referred("cubicBezier", &list, 2_000),
            None,
            48 * MIB,
        ),
        // A long word as a group's type, taken by many tokens: never in a message.
        (
            "a long type inherited",
            inheriting(&"w".repeat(60 * 1024), 40_000),
            None,
            48 * MIB,
        ),
        // A long word as a font weight, referred to by many tokens.
        (
            "a long word referred to",
            referred("fontWeight", &word, 40_000),
            None,
            48 * MIB,
        ),
    ]
}

/// Each shape that meets a bound, what its import says, and the most its read may hold.
#[test]
fn every_shape_that_meets_a_bound_is_read_within_its_memory() {
    purlis_core::unsteered!();
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
    for (shape, bytes, refusal, most) in cases.into_iter().chain(shared_values()) {
        let started = std::time::Instant::now();
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
        if let Ok(imported) = &answer {
            let said: usize = imported
                .left_out
                .iter()
                .map(|it| it.key.len() + it.why.len())
                .sum();
            if said > 16 * 1024 * 1024 {
                over.push(format!(
                    "{shape}: says {said} bytes of what it did not take"
                ));
            }
        }
        // A debug build under a counting allocator: each shape reads in seconds, where a copy for
        // each reference would take minutes and gigabytes.
        if started.elapsed() > std::time::Duration::from_secs(30) {
            over.push(format!("{shape}: took {:?}", started.elapsed()));
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
