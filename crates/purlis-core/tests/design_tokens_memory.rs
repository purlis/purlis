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

/// What an import of a shape must answer.
#[derive(Clone, Copy)]
enum Says {
    /// It refuses the file, with this in its sentence.
    Refused(&'static str),
    /// It reads the file.
    Read,
    /// It reads the file and lists at least this many tokens as not taken, with this as why: so
    /// the shape is known to have reached the bound it is there for.
    LeavesOut(&'static str, usize),
}

/// A shape: its name, its file, what its import says, and the most its read may hold.
type Shape = (&'static str, Vec<u8>, Says, u64);

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

/// Where the tokens that refer to a value are, each read its own way: the text before them and
/// the text after them.
const AMONG_TOKENS: (&str, &str) = ("", "}}}");
const IN_MOTION: (&str, &str) = ("\"motion\":{", "}}}}");
const IN_COMPACT: (&str, &str) = ("", "}},\"compact\":{");

/// An appearance group `dark` holding `base` (a token's JSON text) and, `within` it, `count`
/// tokens of type `kind` (or none of their own, so they take the type of what they refer to) that
/// refer to it.
fn referring(base: &str, within: (&str, &str), kind: Option<&str>, count: usize) -> Vec<u8> {
    let own = kind.map_or(String::new(), |kind| format!("\"$type\":\"{kind}\","));
    let (before, after) = within;
    let mut text = format!("{{\"appearance\":{{\"dark\":{{\"base\":{base}");
    if within == IN_COMPACT {
        text.push_str(after);
    } else if !before.is_empty() {
        text.push(',');
        text.push_str(before);
    }
    for at in 0..count {
        if at > 0 || within == AMONG_TOKENS {
            text.push(',');
        }
        text.push_str(&format!(
            "\"r{at}\":{{{own}\"$value\":\"{{appearance.dark.base}}\"}}"
        ));
    }
    text.push_str(if within == IN_COMPACT { "}}" } else { after });
    text.into_bytes()
}

/// **The shapes of every value kind an import reads that the shapes of shared values do not reach** (#3005):
/// each a long value many tokens refer to, whose bound is that the value is read where it is.
/// Without its bound, each copies the value for every token or parses it again for each.
fn value_kinds() -> Vec<Shape> {
    const COUNT: usize = 40_000;
    const NOT_SHORT: &str = "is not a number of at most";
    const UNKNOWN_TYPE: &str = "a token of a type the format does not have";
    let long = "9".repeat(60 * 1024);
    let word = "w".repeat(60 * 1024);
    let long_px =
        format!("{{\"$type\":\"dimension\",\"$value\":{{\"value\":{long},\"unit\":\"px\"}}}}");
    let long_number = format!("{{\"$type\":\"number\",\"$value\":{long}}}");
    let long_type = format!("{{\"$type\":\"{word}\",\"$value\":1}}");
    // A list of 20,000 values, 500 times over: small enough that a copy for each reference is
    // hundreds of MiB, where the long list of the shapes of shared values would be many GiB, so finding out costs little.
    let list = format!(
        "{{\"$type\":\"number\",\"$value\":[{}0]}}",
        "0,".repeat(20_000)
    );
    // A whole number of ms in seconds, spelled at length: taken, it would be read as 1000 ms.
    let seconds = format!(
        "{{\"$type\":\"duration\",\"$value\":{{\"value\":1.{},\"unit\":\"s\"}}}}",
        "0".repeat(60 * 1024)
    );
    // The same in ms, which a read takes as it stands: without its bound, a copy for each token.
    let millis =
        format!("{{\"$type\":\"duration\",\"$value\":{{\"value\":{long},\"unit\":\"ms\"}}}}");
    vec![
        (
            "a duration in ms with a long number",
            referring(&millis, IN_MOTION, Some("duration"), COUNT),
            Says::LeavesOut("is a duration with no short value", COUNT),
            48 * MIB,
        ),
        (
            "a dimension with a long number",
            referring(&long_px, AMONG_TOKENS, Some("dimension"), COUNT),
            Says::LeavesOut(NOT_SHORT, COUNT),
            48 * MIB,
        ),
        (
            "a number token whose value is a list",
            referring(&list, AMONG_TOKENS, Some("number"), 500),
            Says::LeavesOut(NOT_SHORT, 500),
            8 * MIB,
        ),
        (
            "a duration in seconds with a long number",
            referring(&seconds, IN_MOTION, Some("duration"), COUNT),
            Says::LeavesOut("is a duration with no short value", COUNT),
            48 * MIB,
        ),
        (
            "a long number whose referrers take its type",
            referring(&long_number, AMONG_TOKENS, None, COUNT),
            Says::LeavesOut(NOT_SHORT, COUNT),
            48 * MIB,
        ),
        (
            "a dimension whose referrers take its type",
            referring(&long_px, AMONG_TOKENS, None, COUNT),
            Says::LeavesOut(NOT_SHORT, COUNT),
            48 * MIB,
        ),
        (
            "a duration whose referrers take its type",
            referring(&seconds, IN_MOTION, None, COUNT),
            Says::LeavesOut("is a duration with no short value", COUNT),
            48 * MIB,
        ),
        (
            "a compact token whose referrers take a long number's type",
            referring(&long_number, IN_COMPACT, None, COUNT),
            Says::LeavesOut(NOT_SHORT, COUNT),
            48 * MIB,
        ),
        (
            "a long type referrers take",
            referring(&long_type, AMONG_TOKENS, None, COUNT),
            Says::LeavesOut(UNKNOWN_TYPE, COUNT),
            48 * MIB,
        ),
        (
            "a long type referrers in motion take",
            referring(&long_type, IN_MOTION, None, COUNT),
            Says::LeavesOut(UNKNOWN_TYPE, COUNT),
            48 * MIB,
        ),
        (
            "a long type referrers in compact take",
            referring(&long_type, IN_COMPACT, None, COUNT),
            Says::LeavesOut(UNKNOWN_TYPE, COUNT),
            48 * MIB,
        ),
    ]
}

/// The shapes where one value is reached by many tokens: by a reference, or by a group's type.
/// Each holds what 40,000 small tokens take to read; one copy of the long value for each would
/// be gigabytes.
fn shared_values() -> Vec<Shape> {
    let long = "9".repeat(60 * 1024);
    let word = format!("\"{}\"", "w".repeat(60 * 1024));
    let list = format!("[{}0]", "0,".repeat(100_000));
    vec![
        // A number longer than one may be: refused before it is parsed.
        (
            "a number too long",
            referred("number", &"9".repeat(MOST_SCALAR_BYTES + 1), 0),
            Says::Refused("number longer"),
            MIB,
        ),
        // A long number, referred to by many tokens: read where it is, never copied.
        (
            "a long number referred to",
            referred("number", &long, 40_000),
            Says::Read,
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
            Says::Read,
            48 * MIB,
        ),
        // A long list as an easing, referred to by many tokens: never copied.
        (
            "a long list referred to",
            referred("cubicBezier", &list, 2_000),
            Says::Read,
            48 * MIB,
        ),
        // A long word as a group's type, taken by many tokens: never in a message.
        (
            "a long type inherited",
            inheriting(&"w".repeat(60 * 1024), 40_000),
            Says::Read,
            48 * MIB,
        ),
        // A long word as a font weight, referred to by many tokens.
        (
            "a long word referred to",
            referred("fontWeight", &word, 40_000),
            Says::Read,
            48 * MIB,
        ),
    ]
}

/// Each shape that meets a bound, what its import says, and the most its read may hold.
#[test]
fn every_shape_that_meets_a_bound_is_read_within_its_memory() {
    purlis_core::unsteered!();
    let cases: Vec<Shape> = vec![
        // The value count, at the byte cap: refused before it is parsed.
        (
            "past the value count",
            listed("0,", MOST_BYTES - 16),
            Says::Refused("values"),
            MIB,
        ),
        // Long strings at the byte cap, within the count: the theme.json it makes is too large.
        (
            "long strings at the cap",
            listed(&format!("\"{}\",", "a".repeat(62)), MOST_BYTES - 16),
            Says::Refused("MiB"),
            80 * MIB,
        ),
        // Many small values, just within the count.
        (
            "many small values",
            listed("0,", u64::try_from(MOST_VALUES).unwrap() * 2 - 64),
            Says::Refused("MiB"),
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
            Says::Refused("appearances"),
            MIB,
        ),
        // A carried file past a theme folder's file cap: refused before it is decoded.
        (
            "a carried file too large",
            json!({ "$extensions": { VENDOR: { "version": 1,
                "files": { "fonts/a.woff2": "A".repeat(3 * 1024 * 1024) } } } })
            .to_string()
            .into_bytes(),
            Says::Refused("larger than"),
            8 * MIB,
        ),
        // Names longer than a name may be, nested, over many tokens.
        (
            "long names",
            nested(&"n".repeat(64 * 1024), 20, 200),
            Says::Refused("bytes long"),
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
            Says::Read,
            72 * MIB,
        ),
    ];
    let mut over = Vec::new();
    for (shape, bytes, says, most) in cases
        .into_iter()
        .chain(shared_values())
        .chain(value_kinds())
    {
        let started = std::time::Instant::now();
        assert!(
            bytes.len() as u64 <= MOST_BYTES,
            "{shape} is within the byte cap"
        );
        let (answer, held) = peak(&bytes);
        match (says, &answer) {
            (Says::Refused(said), Err(why)) if why.contains(said) => {}
            (Says::Read, Ok(_)) => {}
            (Says::LeavesOut(said, count), Ok(imported)) => {
                let listed = imported
                    .left_out
                    .iter()
                    .filter(|it| it.why.contains(said))
                    .count();
                if listed < count {
                    over.push(format!("{shape}: {listed} left out as {said:?}"));
                }
            }
            (_, Err(why)) => over.push(format!(
                "{shape}: refused: {}",
                why.chars().take(200).collect::<String>()
            )),
            (_, Ok(_)) => over.push(format!("{shape}: read, and it should have been refused")),
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
