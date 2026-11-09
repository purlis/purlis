// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// What [`Reports::take`] drafts of `evidence` with `known`, from a command, which holds no
/// sign-in, so nothing is sent; or why it drafted nothing.
fn take(known: &Known, evidence: Evidence) -> Result<Draft, Refusal> {
    match Reports::new(known.clone()).take(evidence, &Caller::command()) {
        Outcome::Drafted { draft, .. } => Ok(draft),
        Outcome::Refused(refused) => Err(refused),
        other => panic!("neither drafted nor refused: {other:?}"),
    }
}

fn described(kind: Kind, text: &str, title: Option<&str>, known: &Known) -> Result<Draft, Refusal> {
    take(
        known,
        Evidence::PersonDraft {
            kind,
            text: text.to_string(),
            title: title.map(str::to_string),
        },
    )
}

fn of_panic(record: &Panic, words: Option<&str>, known: &Known) -> Result<Draft, Refusal> {
    take(
        known,
        Evidence::Panic {
            record: record.clone(),
            words: words.map(str::to_string),
        },
    )
}

fn of_sandbox_block(
    block: &crate::sandboxblock::Block,
    harness: Option<crate::harness::Harness>,
    harness_version: Option<&str>,
) -> Result<Draft, Refusal> {
    take(
        &Known::default(),
        Evidence::PurlisFailure(PurlisFailure::SandboxBlock {
            block: *block,
            ran_as: RanAs {
                harness,
                harness_version: harness_version.map(str::to_string),
                persona: PersonaKind::None,
            },
        }),
    )
}

/// A token of the shape `token_kind` knows, built at run time so no credential-shaped literal
/// sits in the source.
fn a_token() -> String {
    format!("ghp_{}", "A1b2C3d4".repeat(5))
}

fn known() -> Known {
    Known {
        plane: Some("/srv/planes/acme-plane".to_string()),
        home: Some("/var/root".to_string()),
        env: vec![
            (
                "DB_PASSWORD_FOR_CI".to_string(),
                "correct-horse-battery".to_string(),
            ),
            ("TERM".to_string(), "xterm-256color".to_string()),
            ("SHORTY".to_string(), "abc".to_string()),
        ],
        names: vec![
            ("acme".to_string(), "[workspace]"),
            ("acme-migration".to_string(), "[workspace]"),
            ("billing-api".to_string(), "[repo]"),
            ("clientvault".to_string(), "[vault]"),
        ],
    }
}

#[test]
fn a_value_in_the_environment_never_reaches_the_report_and_its_name_says_what_went() {
    let (out, used) = known().scrub("it printed correct-horse-battery and stopped");
    assert_eq!(out, "it printed [env $DB_PASSWORD_FOR_CI] and stopped");
    assert!(used.contains(&"environment values".to_string()), "{used:?}");
}

#[test]
fn a_terminal_name_and_a_short_value_are_left_alone() {
    let (out, _) = known().scrub("in xterm-256color, the flag abc did nothing");
    assert_eq!(out, "in xterm-256color, the flag abc did nothing");
}

#[test]
fn a_line_secretshape_reads_as_a_credential_goes_whole() {
    let text = format!("first line\nexport X={}\nlast line", a_token());
    let (out, used) = known().scrub(&text);
    // No failure message prints `out`: a test about secrets does not write one to a log.
    assert!(!out.contains(&a_token()), "the token survived the scrub");
    assert!(
        out == "first line\n[redacted: a line that looks like a token by its forge's prefix]\nlast line",
        "the credential's line was not replaced whole"
    );
    assert!(used.contains(&"lines that look like a credential".to_string()));

    let (out, _) = known().scrub("password = hunter2hunter2");
    assert!(!out.contains("hunter2"), "the password survived the scrub");
}

#[test]
fn the_planes_path_and_home_paths_are_removed_and_the_markdown_around_them_survives() {
    let (out, used) = known().scrub(
        "`/srv/planes/acme-plane/charter.toml` and `/Users/someone/work/x.rs` and \
         /home/who/y and /var/root/z",
    );
    assert_eq!(
        out,
        "`[plane]/charter.toml` and `[home path]` and [home path] and [home]/z"
    );
    assert!(used.contains(&"the plane's path".to_string()), "{used:?}");
    assert!(used.contains(&"home paths".to_string()), "{used:?}");
}

#[test]
fn a_known_name_goes_whole_and_only_as_a_whole_word() {
    let (out, used) = known().scrub(
        "acme-migration broke, acme too, billing-api cloned, clientvault read; acmeish stays",
    );
    assert_eq!(
        out,
        "[workspace] broke, [workspace] too, [repo] cloned, [vault] read; acmeish stays"
    );
    assert!(used.contains(&"workspace names".to_string()), "{used:?}");
    assert!(used.contains(&"repo names".to_string()), "{used:?}");
    assert!(used.contains(&"vault names".to_string()), "{used:?}");
}

#[test]
fn a_planes_names_are_read_from_its_directories_and_default_is_not_one() {
    let dir = tempfile::tempdir().expect("a directory");
    let plane = dir.path();
    for d in [
        "workspaces/acme/billing-api/.git",
        "workspaces/acme/notes",
        "workspaces/default",
        "personas/reviewer",
        "personas/_shared",
    ] {
        std::fs::create_dir_all(plane.join(d)).expect("mkdir");
    }
    let mut names = names_in(plane);
    names.sort();
    assert_eq!(
        names,
        vec![
            ("acme".to_string(), "[workspace]"),
            ("billing-api".to_string(), "[repo]"),
            ("reviewer".to_string(), "[persona]"),
        ]
    );
}

/// ADR 0114: a clone in a workspace's `repos/` is the reporter's as one at its root is, so its
/// name is masked too, and `repos` itself is no repo's name.
#[test]
fn a_clone_in_repos_is_named_as_a_repo_and_repos_is_not() {
    let dir = tempfile::tempdir().expect("a directory");
    let plane = dir.path();
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").expect("manifest");
    for d in [
        "workspaces/acme/repos/billing-api/.git",
        "workspaces/acme/ledger/.git",
    ] {
        std::fs::create_dir_all(plane.join(d)).expect("mkdir");
    }
    let mut names = names_in(plane);
    names.sort();
    assert_eq!(
        names,
        vec![
            ("acme".to_string(), "[workspace]"),
            ("billing-api".to_string(), "[repo]"),
            ("ledger".to_string(), "[repo]"),
        ]
    );
}

/// A clone whose folder name purlis takes for no repo's is still the reporter's: a name too
/// many is masked, a name too few is told.
#[test]
fn a_clone_whose_name_is_no_repos_is_still_masked() {
    let dir = tempfile::tempdir().expect("a directory");
    let plane = dir.path();
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").expect("manifest");
    std::fs::create_dir_all(plane.join("workspaces/acme/Billing Api/.git")).expect("mkdir");
    let names = names_in(plane);
    assert!(
        names.contains(&("Billing Api".to_string(), "[repo]")),
        "{names:?}"
    );
}

#[test]
fn a_described_report_is_titled_by_its_first_line_without_a_heading_marker() {
    let d = described(
        Kind::Feature,
        "## `charter report` should exist\n\nbody",
        None,
        &Known::default(),
    )
    .expect("a draft");
    assert_eq!(d.title, "`charter report` should exist");
    assert!(
        d.body
            .starts_with("## `charter report` should exist\n\nbody\n\n---\npurlis ")
    );
    assert!(
        d.body.ends_with("_Filed with `purlis report feature`._"),
        "the body does not end with its footer"
    );

    let long = "word ".repeat(40);
    let d = described(Kind::Bug, &long, None, &Known::default()).expect("a draft");
    assert!(
        d.title.chars().count() <= TITLE_MAX && d.title.ends_with("word…"),
        "{}",
        d.title
    );
}

#[test]
fn an_empty_description_is_refused() {
    let err = described(Kind::Bug, "  \n ", None, &Known::default()).unwrap_err();
    assert!(
        matches!(&err, Refusal::Unusable(said) if said.contains("needs a description")),
        "{err:?}"
    );
}

#[test]
fn the_title_is_scrubbed_too() {
    let d = described(Kind::Bug, "x", Some("acme broke"), &known()).expect("a draft");
    assert_eq!(d.title, "[workspace] broke");
}

#[test]
fn the_digest_names_exactly_one_draft() {
    let a = described(Kind::Bug, "one", None, &Known::default()).unwrap();
    let b = described(Kind::Bug, "one", None, &Known::default()).unwrap();
    let c = described(Kind::Bug, "one.", None, &Known::default()).unwrap();
    assert_eq!(a.digest(), b.digest());
    assert_ne!(a.digest(), c.digest());
    assert_eq!(a.digest().len(), 12);
}

const LOG: &str = "charter-panic pid 1 at 5 (seconds since 1970)\n\
thread: main\n\
place: app/src-tauri/src/old.rs:1:1\n\
message: the old one\n\
backtrace:\n  0: frame\n\
charter-panic end\n\
charter-panic pid 42 at 9 (seconds since 1970)\n\
version: 9.8.7\n\
thread: charter-view\n\
place: /Users/someone/.cargo/registry/src/index/tokio-1.0/src/rt.rs:12:5\n\
message: workspace acme is gone\nsecond line\n\
backtrace:\n  0: /Users/someone/secret/frame.rs\n\
charter-panic end\n";

#[test]
fn the_newest_panic_in_the_log_is_the_one_read() {
    let p = latest_panic(LOG).expect("a panic");
    assert_eq!(
        p,
        Panic {
            at: Some(9),
            place: "/Users/someone/.cargo/registry/src/index/tokio-1.0/src/rt.rs:12:5".into(),
            message: "workspace acme is gone\nsecond line".into(),
            version: Some("9.8.7".into()),
        }
    );
    let old = latest_panic(&LOG[..LOG.find("charter-panic pid 42").unwrap()]).unwrap();
    assert_eq!(old.version, None);
    assert_eq!(old.place, "app/src-tauri/src/old.rs:1:1");
    assert_eq!(latest_panic("nothing here"), None);
}

#[test]
fn a_panic_becomes_a_bug_with_its_place_and_version_and_nothing_else_of_the_record() {
    let p = latest_panic(LOG).unwrap();
    let d = of_panic(&p, Some("I clicked Save"), &known()).expect("a draft");
    assert_eq!(d.kind, Kind::Bug);
    assert_eq!(d.title, "purlis panicked at …/tokio-1.0/src/rt.rs:12:5");
    assert!(
        d.body.starts_with("I clicked Save\n\npurlis panicked."),
        "{}",
        d.body
    );
    assert!(
        d.body.contains("`…/tokio-1.0/src/rt.rs:12:5`"),
        "{}",
        d.body
    );
    assert!(d.body.contains("**purlis version:** 9.8.7"), "{}", d.body);
    assert!(
        d.body
            .contains("  > workspace [workspace] is gone\n  > second line"),
        "{}",
        d.body
    );
    for gone in ["someone", "frame.rs", "charter-view", "acme"] {
        assert!(
            !d.body.contains(gone),
            "{gone} reached the body: {}",
            d.body
        );
    }
}

#[test]
fn a_panic_is_offered_for_two_weeks_and_not_after() {
    let p = latest_panic(LOG).unwrap();
    assert!(p.is_recent(9 + PANIC_OFFERED_FOR_SECS));
    assert!(!p.is_recent(10 + PANIC_OFFERED_FOR_SECS));
    let undated = Panic { at: None, ..p };
    assert!(!undated.is_recent(9));
}

#[test]
fn charters_own_words_and_the_placeholders_words_are_never_scrubbed_as_names() {
    let dir = tempfile::tempdir().expect("a directory");
    for d in [
        "workspaces/ide/charter/.git",
        "workspaces/ide/charter-app/.git",
        "workspaces/env",
        "personas/steward",
    ] {
        std::fs::create_dir_all(dir.path().join(d)).expect("mkdir");
    }
    let known = Known {
        names: names_in(dir.path()),
        ..Known::default()
    };
    let (out, _) = known.scrub("charter report fails in charter-app for steward");
    assert_eq!(out, "charter report fails in charter-app for steward");
    assert_eq!(known.names, vec![("ide".to_string(), "[workspace]")]);
}

#[test]
fn a_home_path_flattened_into_one_directory_name_is_a_home_path() {
    let (out, _) = Known::default().scrub("under /private/tmp/x/-Users-someone-work-acme/y.rs");
    assert_eq!(out, "under /private/tmp/x/[home path]");
}

#[test]
fn a_title_given_by_hand_is_one_line_and_bounded() {
    let long = format!("first\nsecond {}", "word ".repeat(30));
    let d = described(Kind::Bug, "x", Some(&long), &Known::default()).unwrap();
    assert!(!d.title.contains('\n'), "{}", d.title);
    assert!(d.title.chars().count() <= TITLE_MAX, "{}", d.title);
}

#[test]
fn a_body_too_long_for_a_link_is_left_out_of_it_to_be_pasted() {
    let short = described(Kind::Bug, "short", None, &Known::default()).unwrap();
    let (url, whole) = short.fallback_url();
    assert!(whole && url.contains("&body=short"), "{url}");
    let long = described(Kind::Bug, &"x".repeat(9_000), None, &Known::default()).unwrap();
    let (url, whole) = long.fallback_url();
    assert!(
        !whole && !url.contains("&body=") && url.len() <= LINK_MAX,
        "{}",
        url.len()
    );
}

#[test]
fn a_line_that_spells_a_credential_through_its_escapes_is_redacted() {
    let tail = &a_token()[1..];
    let (out, used) = known().scrub(&format!("before\n{{\"t\": \"\\u0067{tail}\"}}\nafter"));
    assert!(!out.contains(tail), "{out}");
    assert!(out.contains("[redacted: a line that looks like"), "{out}");
    assert!(
        out.starts_with("before\n") && out.ends_with("\nafter"),
        "{out}"
    );
    assert!(
        used.iter()
            .any(|u| u == "lines that look like a credential")
    );
}

#[test]
fn a_credential_split_across_lines_a_backslash_joins_is_redacted_with_its_lines() {
    let token = a_token();
    let text = format!(
        "before\nnote = \"\"\"{}\\\n    {}\"\"\"\nafter",
        &token[..8],
        &token[8..]
    );
    let (out, _) = known().scrub(&text);
    assert!(!out.contains(&token[8..]), "{out}");
    assert!(!out.contains(&token[..8]), "{out}");
    assert!(
        out.starts_with("before\n") && out.ends_with("\nafter"),
        "{out}"
    );
    assert_eq!(
        out.matches("[redacted: a line that looks like").count(),
        2,
        "{out}"
    );
}

// ---- a sandbox block of purlis's own (#1338) ----------------------------------------------------

#[test]
fn a_sandbox_block_draft_names_the_operation_the_kind_and_the_versions_and_nothing_else() {
    use crate::sandboxblock::{Block, Kind as Blocked, Operation, Place, detect};
    // The block as a hook reads it: a real command, real paths, a real program's words.
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "purlis session record --title 'billing-api outage' <<'EOF'\nsecret body\nEOF"},
        "error": "Exit code 1\npurlis: could not save /srv/planes/acme-plane/workspaces/acme/sessions/x.md\n\
                  <sandbox_violations>\npurlis(4021) deny(1) file-write-create \
                  /srv/planes/acme-plane/workspaces/acme/sessions/x.md\n</sandbox_violations>",
    });
    let place = Place {
        root: Path::new("/srv/planes/acme-plane"),
        chat: Path::new("/srv/planes/acme-plane/workspaces/acme/billing-api"),
        cwd: Path::new("/srv/planes/acme-plane/workspaces/acme/billing-api"),
        home: Some(Path::new("/var/root")),
    };
    let blocks = detect(&payload, &place);
    assert_eq!(
        blocks,
        vec![Block {
            operation: Operation::Write,
            kind: Blocked::ProjectFiles,
            ours: true
        }]
    );
    let draft = of_sandbox_block(
        &blocks[0],
        Some(crate::harness::Harness::ClaudeCode),
        Some("2.1.268"),
    )
    .unwrap();
    assert_eq!(draft.kind, Kind::Bug);
    assert_eq!(
        draft.title,
        "Sandbox blocked purlis's own write (project-files)"
    );
    for said in [
        "- **operation:** write",
        "- **kind of path or host:** project-files (the project's own files)",
        "- **harness:** claude",
        "- **harness version:** 2.1.268",
        &format!("- **purlis version:** {}", crate::adopt::app_version()),
        std::env::consts::OS,
    ] {
        assert!(
            draft.body.contains(said),
            "{said} missing from:\n{}",
            draft.body
        );
    }
    // No path, argument or content: nothing of the command, its paths or what it printed.
    for leak in [
        "/srv",
        "/var/root",
        "acme",
        "billing",
        "outage",
        "secret",
        "sessions",
        "x.md",
        "4021",
        "--title",
        "EOF",
    ] {
        assert!(
            !format!("{}\n{}", draft.title, draft.body).contains(leak),
            "{leak} leaked into:\n{}\n{}",
            draft.title,
            draft.body
        );
    }
    assert!(draft.scrubbed.is_empty());
}

#[test]
fn a_sandbox_block_draft_with_no_harness_known_says_so() {
    let block = crate::sandboxblock::Block {
        operation: crate::sandboxblock::Operation::Connect,
        kind: crate::sandboxblock::Kind::Host,
        ours: true,
    };
    let draft = of_sandbox_block(&block, None, None).unwrap();
    assert!(
        draft.body.contains("- **harness:** not known"),
        "{}",
        draft.body
    );
    assert_eq!(
        draft.digest(),
        of_sandbox_block(&block, None, None).unwrap().digest()
    );
}

/// #1353: the harness's version is in the draft only as a version's shape: digits and dots,
/// as the app heard the harness answer `--version`. Anything else, or nothing heard, reads
/// "not known", so no text from a program can reach the draft through it.
#[test]
fn a_sandbox_block_draft_names_the_harness_version_only_in_a_versions_shape() {
    let block = crate::sandboxblock::Block {
        operation: crate::sandboxblock::Operation::Write,
        kind: crate::sandboxblock::Kind::ProjectFiles,
        ours: true,
    };
    let claude = Some(crate::harness::Harness::ClaudeCode);
    let draft = of_sandbox_block(&block, claude, Some("2.1.268")).unwrap();
    assert!(
        draft.body.contains("- **harness version:** 2.1.268\n"),
        "{}",
        draft.body
    );
    for said in [
        None,
        Some(""),
        Some("2.1.268 (Claude Code)"),
        Some("/Users/dev/secret"),
        Some("1..2"),
        Some(".1"),
        Some("v2"),
        Some("1.2.3.4.5.6.7.8.9.10.11.12.13.14.15.16.17"),
    ] {
        let draft = of_sandbox_block(&block, claude, said).unwrap();
        assert!(
            draft.body.contains("- **harness version:** not known\n"),
            "{said:?}: {}",
            draft.body
        );
    }
}

#[test]
fn the_samples_are_what_the_envelope_builders_make_of_a_sample_panic_and_draft() {
    let samples = samples();

    let event = |envelope: &str| -> serde_json::Value {
        let lines: Vec<&str> = envelope.lines().collect();
        assert_eq!(lines.len(), 3, "an envelope of one event: {envelope}");
        serde_json::from_str(lines[2]).expect("the event")
    };
    let crash = event(&samples.crash);
    assert_eq!(crash["exception"]["values"][0]["type"], "panic");
    assert_eq!(crash["level"], "fatal");
    assert_eq!(crash["fingerprint"].as_array().map(Vec::len), Some(1));
    assert_eq!(crash["tags"]["fingerprint"], crash["fingerprint"][0]);
    assert!(crash.get("user").is_none() && crash.get("server_name").is_none());

    let draft = event(&samples.draft);
    assert_eq!(draft["tags"]["written_by"], "agent");
    assert_eq!(draft["tags"]["reviewed_by_the_person"], false);
    assert_eq!(draft["tags"]["persona_kind"], "template");
    let text = draft["message"]["formatted"].as_str().unwrap_or_default();
    assert!(
        text.contains("a persona from a purlis template"),
        "its footer says what the chat ran under: {text}"
    );
}
