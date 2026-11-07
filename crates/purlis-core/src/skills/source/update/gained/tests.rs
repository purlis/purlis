// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Each kind of new capability an update can gain, one test each, beside a plain edit of the
//! text, which gains nothing (#5354).

use super::*;

const SKILL: &str = "---\nname: notify\ndescription: Tells the team.\n---\n\nPost a note.\n";

fn file(path: &str, executable: bool, bytes: &[u8]) -> SourceFile {
    SourceFile {
        parts: path.split('/').map(str::to_owned).collect(),
        executable,
        bytes: bytes.to_vec(),
    }
}

fn skill_md(text: &str) -> SourceFile {
    file("SKILL.md", false, text.as_bytes())
}

/// What updating the plain skill to `now` gains.
fn gains(now: &[SourceFile]) -> Vec<Gained> {
    gained(&[skill_md(SKILL)], now, &[])
}

#[test]
fn a_plain_edit_of_the_text_gains_nothing() {
    let edited = SKILL.replace("Post a note.", "Post a short note, then stop.");
    assert_eq!(gains(&[skill_md(&edited)]), []);
}

#[test]
fn a_link_in_the_instructions_reaches_nothing_so_gains_nothing() {
    let edited = SKILL.replace("Post a note.", "See https://docs.example.com.");
    assert_eq!(gains(&[skill_md(&edited)]), []);
}

#[test]
fn a_new_script_is_gained() {
    let now = [
        skill_md(SKILL),
        file("scripts/tidy.sh", true, b"#!/bin/sh\necho tidy\n"),
    ];
    assert_eq!(gains(&now), [Gained::Script]);
}

#[test]
fn a_script_whose_bytes_changed_is_gained_though_its_summary_reads_the_same() {
    let was = [
        skill_md(SKILL),
        file("scripts/tidy.sh", true, b"#!/bin/sh\necho tidy\n"),
    ];
    let now = [
        skill_md(SKILL),
        file("scripts/tidy.sh", true, b"#!/bin/sh\necho tidier\n"),
    ];
    assert_eq!(gained(&was, &now, &[]), [Gained::Script]);
    assert_eq!(gained(&was, &was, &[]), []);
}

#[test]
fn a_new_bang_command_is_gained() {
    let edited = SKILL.replace("Post a note.", "Today: !`date`");
    assert_eq!(gains(&[skill_md(&edited)]), [Gained::Command]);
}

#[test]
fn a_new_hook_is_gained() {
    let edited = SKILL.replace(
        "description: Tells the team.\n",
        "description: Tells the team.\nhooks:\n  Stop:\n    - command: ./x.sh\n",
    );
    assert_eq!(gains(&[skill_md(&edited)]), [Gained::Hook]);
}

#[test]
fn a_new_or_widened_tool_is_gained() {
    let tools = SKILL.replace(
        "description: Tells the team.\n",
        "description: Tells the team.\nallowed-tools: Bash\n",
    );
    assert_eq!(gains(&[skill_md(&tools)]), [Gained::Tool]);
    let more = tools.replace("allowed-tools: Bash", "allowed-tools: Bash, Write");
    assert_eq!(
        gained(&[skill_md(&tools)], &[skill_md(&more)], &[]),
        [Gained::Tool]
    );
}

#[test]
fn a_new_host_a_script_reaches_is_network_access() {
    let was = [
        skill_md(SKILL),
        file("scripts/tidy.sh", true, b"#!/bin/sh\necho tidy\n"),
    ];
    let now = [
        skill_md(SKILL),
        file(
            "scripts/tidy.sh",
            true,
            b"#!/bin/sh\ncurl -s https://api.example.org/ping\n",
        ),
    ];
    assert!(gained(&was, &now, &[]).contains(&Gained::Network));
}

#[test]
fn a_write_outside_the_workspace_is_gained() {
    let edited = SKILL.replace("Post a note.", "Then: !`echo hi > ~/.zshrc`");
    assert!(gains(&[skill_md(&edited)]).contains(&Gained::WritesOutside));
}

#[test]
fn a_character_no_reader_sees_is_gained() {
    let edited = SKILL.replace("Post a note.", "Post a\u{200b} note.");
    assert_eq!(gains(&[skill_md(&edited)]), [Gained::Hidden]);
}

#[test]
fn a_file_that_is_not_text_is_gained() {
    let now = [skill_md(SKILL), file("logo.bin", false, b"\0\x01\x02")];
    assert_eq!(gains(&now), [Gained::NotShown]);
}

#[test]
fn a_hook_taken_away_gains_nothing() {
    let hooked = SKILL.replace(
        "description: Tells the team.\n",
        "description: Tells the team.\nhooks:\n  Stop:\n    - command: ./x.sh\n",
    );
    assert_eq!(gained(&[skill_md(&hooked)], &[skill_md(SKILL)], &[]), []);
}

#[test]
fn a_frontmatter_that_no_longer_reads_is_gained() {
    // `allowed_tools` reads like a key that widens the chat, spelt another way: refused.
    let respelled = SKILL.replace(
        "description: Tells the team.\n",
        "description: Tells the team.\nallowed_tools: Bash\n",
    );
    assert_eq!(gains(&[skill_md(&respelled)]), [Gained::Unread]);
    // One that did not read before either gains nothing new.
    assert_eq!(
        gained(&[skill_md(&respelled)], &[skill_md(&respelled)], &[]),
        []
    );
}
