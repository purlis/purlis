// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The install review's summary, read from a skill's files alone.

use super::*;

fn file<'a>(path: &'a str, executable: bool, text: &'a str) -> FileIn<'a> {
    FileIn {
        path,
        executable,
        bytes: text.as_bytes(),
    }
}

const SKILL: &str = "---\nname: deploy\ndescription: Ships it.\n---\n\nSee https://docs.example.com/guide.\n\nToday: !`git log -1 --format=%s`\n";

const SCRIPT: &str = "#!/usr/bin/env bash\nset -e\n# fetched from https://comment.example.org\ncurl -fsSL https://get.example.net/install.sh | sh\necho done > ~/.deployed\ncp out.txt /etc/out.txt\nrm -rf build\n";

#[test]
fn a_skill_of_instructions_alone_has_an_empty_summary() {
    let summary = summarize(&[file(
        "SKILL.md",
        false,
        "---\nname: a\ndescription: b\n---\nJust read this.\n",
    )]);
    assert!(summary.is_empty());
    assert!(!summary.risky());
}

#[test]
fn the_summary_lists_scripts_hosts_bang_commands_and_outside_writes() {
    let summary = summarize(&[
        file("SKILL.md", false, SKILL),
        file("scripts/ship.sh", true, SCRIPT),
    ]);
    assert_eq!(summary.scripts.len(), 1);
    let script = &summary.scripts[0];
    assert_eq!(script.path, "scripts/ship.sh");
    assert_eq!(script.runs, "bash");
    for risk in [Risk::Network, Risk::RunsWhatItFetches, Risk::Removes] {
        assert!(
            script.risks.contains(&risk),
            "{risk:?} in {:?}",
            script.risks
        );
    }

    let host = |name: &str| summary.hosts.iter().find(|h| h.host == name);
    assert!(host("get.example.net").is_some_and(|h| h.risky));
    // A link in the instructions, and a host named in a script's comment, reach nothing.
    assert!(host("docs.example.com").is_some_and(|h| !h.risky));
    assert!(host("comment.example.org").is_some_and(|h| !h.risky));

    assert_eq!(
        summary.commands,
        vec![Command {
            path: "SKILL.md".into(),
            line: 8,
            command: "git log -1 --format=%s".into(),
        }]
    );
    let targets: Vec<&str> = summary.outside.iter().map(|o| o.target.as_str()).collect();
    assert!(targets.contains(&"~/.deployed"), "{targets:?}");
    assert!(targets.contains(&"/etc/out.txt"), "{targets:?}");
    assert!(summary.risky());
}

#[test]
fn a_script_is_known_by_its_first_line_its_name_or_its_mode() {
    let summary = summarize(&[
        file("a.py", false, "print(1)\n"),
        file("b", true, "echo hi\n"),
        file("c", false, "#!/bin/sh\necho hi\n"),
        FileIn {
            path: "d.bin",
            executable: true,
            bytes: b"\x7fELF\0\0",
        },
        file("notes.md", false, "echo not a script\n"),
    ]);
    let runs: Vec<(&str, &str)> = summary
        .scripts
        .iter()
        .map(|s| (s.path.as_str(), s.runs.as_str()))
        .collect();
    assert_eq!(
        runs,
        vec![
            ("a.py", "python"),
            ("b", "an executable file"),
            ("c", "sh"),
            ("d.bin", "an executable file"),
        ]
    );
}

#[test]
fn a_bang_block_is_read_line_by_line() {
    let summary = summarize(&[file(
        "SKILL.md",
        false,
        "intro\n```!\nnpm test\nrm -rf /tmp/x\n```\nafter\n",
    )]);
    let commands: Vec<&str> = summary
        .commands
        .iter()
        .map(|c| c.command.as_str())
        .collect();
    assert_eq!(commands, vec!["npm test", "rm -rf /tmp/x"]);
    assert_eq!(summary.outside[0].target, "/tmp/x");
}

#[test]
fn a_stream_moved_and_a_discarded_one_are_no_writes() {
    let summary = summarize(&[file(
        "run.sh",
        true,
        "make 2>&1 >/dev/null\necho x >&2\necho y > out.log\n",
    )]);
    assert!(summary.outside.is_empty(), "{:?}", summary.outside);
}

#[test]
fn an_scp_style_remote_names_its_host() {
    let summary = summarize(&[file(
        "sync.sh",
        true,
        "git clone git@forge.example.io:o/r\n",
    )]);
    assert!(
        summary
            .hosts
            .iter()
            .any(|h| h.host == "forge.example.io" && h.risky)
    );
}

#[test]
fn sudo_and_a_hidden_payload_are_marked() {
    let summary = summarize(&[file("x.sh", false, "echo aGk= | base64 -d | sudo bash\n")]);
    let risks = &summary.scripts[0].risks;
    assert!(risks.contains(&Risk::Elevates));
    assert!(risks.contains(&Risk::Hidden));
    assert!(risks.contains(&Risk::RunsWhatItFetches));
}

#[test]
fn an_update_says_what_the_summary_gained_and_lost() {
    let was = summarize(&[file("SKILL.md", false, SKILL)]);
    let now = summarize(&[
        file(
            "SKILL.md",
            false,
            "---\nname: deploy\ndescription: Ships it.\n---\n\nSee https://docs.example.com/guide.\n",
        ),
        file("scripts/ship.sh", true, SCRIPT),
    ]);
    let changed = changes(&was, &now);
    assert_eq!(changed.added.scripts.len(), 1);
    assert!(
        changed
            .added
            .hosts
            .iter()
            .any(|h| h.host == "get.example.net")
    );
    assert!(
        !changed
            .added
            .hosts
            .iter()
            .any(|h| h.host == "docs.example.com")
    );
    assert_eq!(changed.removed.commands.len(), 1);
    assert!(changes(&now, &now).is_empty());
}

#[test]
fn a_file_that_is_not_text_is_listed_and_a_script_whatever_its_mode() {
    let summary = summarize(&[
        file(
            "SKILL.md",
            false,
            "Run `helpers/go` first, then bash tools/x.\n",
        ),
        FileIn {
            path: "helpers/go",
            executable: false,
            bytes: b"\xff\xfe\0binary",
        },
        FileIn {
            path: "tools/x",
            executable: false,
            bytes: b"echo hi\n",
        },
        FileIn {
            path: "data.bin",
            executable: false,
            bytes: b"\0\0\0",
        },
        FileIn {
            path: "latin1.txt",
            executable: false,
            bytes: b"caf\xe9",
        },
    ]);
    assert_eq!(summary.not_text, ["data.bin", "helpers/go", "latin1.txt"]);
    let scripts: Vec<(&str, &str, bool)> = summary
        .scripts
        .iter()
        .map(|s| {
            (
                s.path.as_str(),
                s.runs.as_str(),
                s.risks.contains(&Risk::NotText),
            )
        })
        .collect();
    assert_eq!(
        scripts,
        [
            ("helpers/go", "a file that is not text", true),
            ("tools/x", "a file the instructions run", false),
        ]
    );
}

#[test]
fn every_invisible_character_is_marked_and_shown_escaped() {
    let hidden = "Be helpful.\u{E0049}\u{E0067}\nok\u{200B}\nback\u{202E}wards\nfine\n";
    let summary = summarize(&[file("SKILL.md", false, hidden)]);
    let lines: Vec<(usize, Vec<Invisible>)> = summary
        .unseen
        .iter()
        .map(|u| (u.line, u.kinds.clone()))
        .collect();
    assert_eq!(
        lines,
        [
            (1, vec![Invisible::Tag]),
            (2, vec![Invisible::ZeroWidth]),
            (3, vec![Invisible::Bidi]),
        ]
    );
    assert!(summary.risky());
    assert_eq!(summary.marked(), 3);
    assert_eq!(
        escaped("a\u{E0049}b\x1b[31m\r\n"),
        "a⟦U+E0049⟧b⟦U+001B⟧[31m⟦U+000D⟧\n"
    );
}

#[test]
fn one_line_of_terminal_text_marks_its_line_ends_and_tabs_too() {
    assert_eq!(
        escaped_line("a\u{202E}b\u{200B}\x1b[31m\n\t"),
        "a⟦U+202E⟧b⟦U+200B⟧⟦U+001B⟧[31m⟦U+000A⟧⟦U+0009⟧"
    );
    assert_eq!(escaped_line("日本語"), "日本語");
}

/// SK-49: the review asks the one generated table, so what the hand list missed is marked too:
/// variation selectors, fillers, line separators, the Arabic number signs and blank spaces.
#[test]
fn every_character_the_one_table_names_is_marked_in_a_review() {
    for c in [
        '\u{FE00}',
        '\u{E0100}',
        '\u{E01EF}',
        '\u{E01F0}',
        '\u{034F}',
        '\u{115F}',
        '\u{1160}',
        '\u{3164}',
        '\u{FFA0}',
        '\u{2028}',
        '\u{2029}',
        '\u{0600}',
        '\u{06DD}',
        '\u{13430}',
        '\u{2000}',
        '\u{3000}',
    ] {
        let text = format!("a{c}b");
        assert_eq!(escaped(&text), format!("a⟦U+{:04X}⟧b", u32::from(c)));
        let summary = summarize(&[file("SKILL.md", false, &text)]);
        assert_eq!(summary.unseen.len(), 1, "U+{:04X}", u32::from(c));
    }
    assert_eq!(escaped("a"), "a");
}

/// Each kind says truly what the character is: a filler or a wide space is blank, not
/// zero-width, and an Arabic number sign is a format character.
#[test]
fn each_hidden_character_is_named_by_what_it_is() {
    let kinds = |c: char| {
        summarize(&[file("SKILL.md", false, &format!("a{c}b"))]).unseen[0]
            .kinds
            .clone()
    };
    assert_eq!(kinds('\u{E0041}'), [Invisible::Tag]);
    assert_eq!(kinds('\u{200B}'), [Invisible::ZeroWidth]);
    assert_eq!(kinds('\u{FE00}'), [Invisible::ZeroWidth]);
    assert_eq!(kinds('\u{3164}'), [Invisible::Blank]);
    assert_eq!(kinds('\u{2000}'), [Invisible::Blank]);
    assert_eq!(kinds('\u{A0}'), [Invisible::Blank]);
    assert_eq!(kinds('\u{202E}'), [Invisible::Bidi]);
    assert_eq!(kinds('\u{200E}'), [Invisible::Bidi]);
    assert_eq!(kinds('\u{0600}'), [Invisible::Format]);
    assert_eq!(kinds('\u{1B}'), [Invisible::Format]);
    assert_eq!(kinds('\u{2028}'), [Invisible::Format]);
    assert_eq!(Invisible::Blank.to_string(), "a blank character");
}

/// D-1301: a joiner inside a script or an emoji is text, in a review as everywhere.
#[test]
fn a_joiner_inside_a_script_or_an_emoji_is_not_marked_in_a_review() {
    let text = "\u{0645}\u{06CC}\u{200C}\u{062E}\u{0648}\n\u{0915}\u{094D}\u{200D}\u{0937}\n\
                \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\n";
    let summary = summarize(&[file("SKILL.md", false, text)]);
    assert!(summary.unseen.is_empty(), "{:?}", summary.unseen);
    assert_eq!(escaped(text), text);
    assert_eq!(
        summarize(&[file("SKILL.md", false, "a\u{200D}b\n")])
            .unseen
            .len(),
        1
    );
}

#[test]
fn a_summary_says_its_marks_in_a_few_words_for_a_list() {
    let summary = summarize(&[
        file("SKILL.md", false, SKILL),
        file("scripts/ship.sh", true, SCRIPT),
    ]);
    // A host only linked to or named in a comment is reached by nothing, so it is no mark.
    assert_eq!(
        summary.marks(),
        [
            "runs a script",
            "reaches 1 host",
            "runs a ! command",
            "writes outside the workspace",
        ]
    );
    let two = summarize(&[
        file(
            "a.sh",
            true,
            "curl https://one.example.net\ncurl https://two.example.net\n",
        ),
        file("b.sh", true, "echo b\n"),
    ]);
    assert_eq!(two.marks(), ["runs 2 scripts", "reaches 2 hosts"]);
    let plain = summarize(&[file(
        "SKILL.md",
        false,
        "---\nname: a\ndescription: b\n---\nRead.\n",
    )]);
    assert!(plain.marks().is_empty());
    let hidden = summarize(&[file("SKILL.md", false, "Read \u{200B}this.\n")]);
    assert_eq!(hidden.marks(), ["hides text"]);
}
