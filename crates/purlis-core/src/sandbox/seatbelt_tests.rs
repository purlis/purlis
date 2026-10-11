// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #1425: a start whose words and environment together pass what the system lets a program
//! start with is refused in plain words before the system answers `E2BIG`.

use super::{Exec, TOO_LONG, exec_size, fits_exec, not_started};

const MAC: Exec = Exec {
    total: 1024 * 1024,
    word: None,
};

const LINUX: Exec = Exec {
    total: 2 * 1024 * 1024,
    word: Some(128 * 1024),
};

fn words(first_message: usize) -> Vec<String> {
    vec![
        "-p".to_owned(),
        "(version 1)\n(deny default)\n".to_owned(),
        "/usr/local/bin/codex".to_owned(),
        "exec".to_owned(),
        "x".repeat(first_message),
    ]
}

fn env(bytes: usize) -> Vec<(String, String)> {
    vec![
        ("HOME".to_owned(), "/Users/dev".to_owned()),
        ("BIG".to_owned(), "y".repeat(bytes)),
    ]
}

#[test]
fn an_ordinary_start_fits() {
    let (args, env) = (words(2_000), env(10_000));
    assert_eq!(fits_exec("/usr/bin/sandbox-exec", &args, &env, MAC), Ok(()));
    assert_eq!(
        fits_exec("/usr/bin/sandbox-exec", &args, &env, LINUX),
        Ok(())
    );
}

#[test]
fn a_first_message_past_what_the_system_takes_is_refused_before_it_starts() {
    let args = words(900 * 1024);
    let env = env(200 * 1024);
    assert_eq!(
        fits_exec("/usr/bin/sandbox-exec", &args, &env, MAC),
        Err(TOO_LONG)
    );
    // The same words with a small environment fit: the environment is counted.
    assert_eq!(
        fits_exec("/usr/bin/sandbox-exec", &args, &self::env(10), MAC),
        Ok(())
    );
}

#[test]
fn every_string_is_counted_with_its_end_and_its_pointer() {
    let args = vec!["ab".to_owned()];
    let env = vec![("K".to_owned(), "v".to_owned())];
    // "prog\0", "ab\0", "K=v\0", and a pointer for each, plus each list's closing null.
    let want = 5 + 3 + 4 + 8 * 3 + 8 * 2;
    assert_eq!(exec_size("prog", &args, &env), want);
}

#[test]
fn one_word_past_what_linux_takes_alone_is_refused_however_short_the_rest() {
    let args = words(129 * 1024);
    assert_eq!(
        fits_exec("/usr/bin/bwrap", &args, &env(10), LINUX),
        Err(TOO_LONG)
    );
    assert_eq!(
        fits_exec("/usr/bin/sandbox-exec", &args, &env(10), MAC),
        Ok(())
    );
    // An environment value is one such string too.
    assert_eq!(
        fits_exec("/usr/bin/bwrap", &words(10), &env(129 * 1024), LINUX),
        Err(TOO_LONG)
    );
}

#[test]
fn the_refusal_says_what_happened_and_the_way_on_in_plain_words() {
    let said = not_started(
        "this project runs every chat sandboxed, and",
        TOO_LONG,
        true,
    );
    assert!(said.contains("so nothing was started"), "{said}");
    assert!(said.contains("first message"), "{said}");
    assert!(!said.contains("E2BIG"), "{said}");
    // Never an opt-out: the sandbox is not what is too long.
    assert!(!said.contains("without the sandbox"), "{said}");
}
