// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::ffi::OsString;

use super::*;

fn pairs(list: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    list.iter()
        .map(|(k, v)| (OsString::from(k), OsString::from(v)))
        .collect()
}

#[test]
fn what_is_carried_is_read_back_exactly_in_its_order() {
    let meant = pairs(&[
        ("PATH", "/usr/bin:/bin"),
        ("A", "x=y=z"),
        ("EMPTY", ""),
        ("A", "later"),
    ]);
    let own = carried(meant.clone());
    assert_eq!(read(own), Ok(meant));
}

#[test]
fn nothing_the_wrap_starts_with_is_a_variable_the_command_names() {
    let own = carried(pairs(&[("LD_PRELOAD", "/x.so"), ("GCONV_PATH", "/y")]));
    for (name, _) in &own {
        let name = name.to_str().expect("text");
        assert!(name == COUNT || name.starts_with(EACH), "{name}");
    }
}

#[test]
fn an_empty_environment_is_carried_as_one() {
    assert_eq!(
        read(carried(Vec::<(OsString, OsString)>::new())),
        Ok(Vec::new())
    );
}

#[test]
fn what_the_terminal_sets_beside_what_is_carried_is_let_be_and_not_handed_on() {
    let mut own = carried(pairs(&[("A", "1")]));
    own.push(("TERM".into(), "xterm-256color".into()));
    own.push(("SHELL".into(), "/bin/sh".into()));
    assert_eq!(read(own), Ok(pairs(&[("A", "1")])));
}

#[test]
fn a_wrap_started_with_an_environment_purlis_did_not_build_reads_none() {
    // Inherited whole: nothing carried at all.
    assert_eq!(
        read(pairs(&[("PATH", "/bin"), ("HOME", "/h")])),
        Err(NotBuilt)
    );
    // Carried, and something else beside it.
    let mut beside = carried(pairs(&[("A", "1")]));
    beside.push(("LD_PRELOAD".into(), "/x.so".into()));
    assert_eq!(read(beside), Err(NotBuilt));
    // A piece missing, one too many, or one that is not a variable.
    let mut short = carried(pairs(&[("A", "1"), ("B", "2")]));
    short.retain(|(k, _)| k != "PURLIS_SANDBOX_ENV_1");
    assert_eq!(read(short), Err(NotBuilt));
    let mut long = carried(pairs(&[("A", "1")]));
    long.push(("PURLIS_SANDBOX_ENV_1".into(), "B=2".into()));
    assert_eq!(read(long), Err(NotBuilt));
    let mut bare = carried(pairs(&[("A", "1")]));
    bare[1].1 = "no-equals".into();
    assert_eq!(read(bare), Err(NotBuilt));
    let mut nameless = carried(pairs(&[("A", "1")]));
    nameless[1].1 = "=1".into();
    assert_eq!(read(nameless), Err(NotBuilt));
    let mut counted = carried(pairs(&[("A", "1")]));
    counted[0].1 = "one".into();
    assert_eq!(read(counted), Err(NotBuilt));
}
